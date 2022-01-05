use near_sdk::{ env, ext_contract, near_bindgen, AccountId, setup_alloc, Balance, 
                PanicOnDefault, Gas,  PromiseResult, Promise, serde_json::{json}};
use near_sdk::collections::{UnorderedMap};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Serialize, Deserialize};
use near_sdk::json_types::{ValidAccountId};
use std::fmt::{Debug};
use std::collections::{HashSet};
// use std::convert::TryFrom;

#[allow(dead_code)]
const YOCTO_NEAR: u128 = 1000000000000000000000000;
#[allow(dead_code)]
const STORAGE_PRICE_PER_BYTE: Balance = 10_000_000_000_000_000_000;
const MAX_JUDGES: u8 = 50;
#[allow(dead_code)]
const MAX_EPOCHS_FOR_OPEN_DISPUTES: u64 = 6; // 1 epoch = 12h. 3 days 
#[allow(dead_code)]
const NO_DEPOSIT: Balance = 0;
#[allow(dead_code)]
const BASE_GAS: Gas = 300_000_000_000_000;
//const NANO_SECONDS: u32 = 1_000_000_000;
const ONE_DAY: u64 = 86400000000000;

setup_alloc!();

pub type DisputeId = u64;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Debug, Hash, Eq, PartialOrd, PartialEq, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Vote {
    // Miembro del jurado que emite el voto
    account: AccountId,
    // Decision tomada 
    vote: bool,
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum DisputeStatus {
    Open,
    Resolving,
    Executable,
    Finished
}

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone, Debug)]
#[serde(crate = "near_sdk::serde")]
pub struct Dispute {
    // Identificador para cada disputa
    id: DisputeId,
    service_id: u64,
    // Lista de miembros del jurado y sus respectivos services a retirar
    votes: HashSet<Vote>,
    dispute_status: DisputeStatus,
    // Tiempos
    initial_time_stamp: u64,
    finish_time_stamp: Option<u64>, //Time
    // Partes
    applicant: AccountId, // Empleador demandante
    accused: AccountId, // Profesional acusado
    winner: Option<AccountId>,
    // Pruebas
    applicant_proves: String, // Un markdown con las pruebas
    accused_proves: Option<String> // Un markdown con las pruebas
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Mediator {
    disputes: UnorderedMap<DisputeId, Dispute>,
    disputes_counter: u64,
    owner: AccountId,
    admins: Vec<AccountId>,
    marketplace_account_id: AccountId
}

#[near_bindgen]
impl Mediator {
    #[init]
    pub fn new(marketplace_account_id: AccountId) -> Self{
        if env::state_exists() {
            env::panic("Contract already inicialized".as_bytes());
        }
        let this = Self {
            disputes: UnorderedMap::new(b"d"),
            disputes_counter: 0,
            owner: env::signer_account_id(),
            admins: Vec::new(),
            marketplace_account_id: marketplace_account_id
        };
        return this;
    }

    //////////////////////////////////////
    ///        CORE FUNCTIONS          ///
    //////////////////////////////////////

    #[payable]
    pub fn new_dispute(&mut self, contract_ma: AccountId, method_name: String, service_id: u64, accused: ValidAccountId, proves: String) -> Promise {
        let sender = env::predecessor_account_id();

        let dispute = Dispute {
            id: self.disputes_counter.clone(),
            service_id: service_id,
            votes: HashSet::new(),
            dispute_status: DisputeStatus::Open,
            initial_time_stamp: env::block_timestamp(),
            finish_time_stamp: None,
            applicant: sender,
            accused: accused.to_string(),
            winner: None,
            applicant_proves: proves,
            accused_proves: None
        };
        env::log(format!("{:?}", dispute).as_bytes());
        
        self.disputes.insert(&dispute.id, &dispute);

        self.disputes_counter += 1;
        
        Promise::new(contract_ma).function_call(
            method_name.into_bytes(),
            json!({ "service_id": service_id }).to_string().as_bytes().to_vec(),
            NO_DEPOSIT,
            BASE_GAS,
        )
    }

    #[allow(unused_must_use)]
    pub fn add_accused_proves(&mut self, dispute_id: DisputeId, accused_proves: String) -> Dispute {
        let mut dispute = self.update_dispute_status(dispute_id);
        if dispute.dispute_status != DisputeStatus::Open {
            env::panic(b"Time to upload proves is over");
        }

        // Verificar que sea la persona acusada
        let sender = env::predecessor_account_id();
        if sender != dispute.accused {
            env::panic(b"Address without permissions to upload proves")
        };

        // Verificar que no haya subido ya las pruebas
        if dispute.accused_proves.is_some() {
            env::panic(b"You already upload the proves!");
        }

        dispute.accused_proves.insert(accused_proves);
        dispute.dispute_status = DisputeStatus::Resolving;

        self.disputes.insert(&dispute_id, &dispute);

        return dispute;
    }

    /// Emitir un voto
    /// Solo para miembros del jurado de la misma categoria del servicio en disputa
    /// Se requiere cumplir con un minimo de tokens bloqueados
    /// 
    pub fn vote(&mut self, dispute_id: DisputeId, vote: bool) -> Dispute {
        let sender = env::predecessor_account_id();
        let mut dispute = self.update_dispute_status(dispute_id);

        if dispute.dispute_status != DisputeStatus::Resolving {
            env::panic(b"You cannot vote when the status is different from resolving");
        }

        // Verificar que sea miembro del jurado

        // Verificar que no haya ya votado
        if !dispute.votes.insert(Vote {
            account: sender.clone(),
            vote: vote
        }) {
            env::panic(b"You already vote");
        }

        // Si se completan los votos se pasa la siguiente etapa
        if dispute.votes.len() == MAX_JUDGES as usize {
            dispute.dispute_status = DisputeStatus::Executable
        }

        self.disputes.insert(&dispute_id, &dispute);

        return dispute;
    }

    /// Para verificar y actualizar el estado de la disputa
    /// 
    pub fn update_dispute_status(&mut self, dispute_id: DisputeId) -> Dispute {
        let mut dispute = expect_value_found(self.disputes.get(&dispute_id), "Disputa no encontrada".as_bytes());

        let actual_time = env::block_timestamp();

        // Open is 4 epochs, resolve 8 epochs and execute 1 epoch, finish 0 epoch
        // el perido de open sera de 5 dias y resolving

        // Actualizar por tiempo
        if actual_time >= (dispute.initial_time_stamp + (ONE_DAY * 5)) && (dispute.dispute_status == DisputeStatus::Open) {
            dispute.dispute_status = DisputeStatus::Resolving;
        }

        if (actual_time >= (dispute.initial_time_stamp + (ONE_DAY * 7))) && (dispute.dispute_status == DisputeStatus::Resolving) {
            dispute.dispute_status = DisputeStatus::Executable;
        }

        if dispute.dispute_status == DisputeStatus::Executable {
            let mut agains_votes_counter = 0;
            let mut pro_votes_counter = 0;
            for v in dispute.votes.iter() {
                if v.vote {
                    pro_votes_counter += 1;
                }
                else {
                    agains_votes_counter += 1;
                }
            }

            if pro_votes_counter == agains_votes_counter {
                dispute.dispute_status = DisputeStatus::Open;
            }
            else {
                dispute.dispute_status = DisputeStatus::Finished;
                if pro_votes_counter > agains_votes_counter {
                    dispute.winner = Some(dispute.applicant.clone());
                }
                else {
                    dispute.winner = Some(dispute.accused.clone());
                }

                dispute.finish_time_stamp = Some(env::block_timestamp());

                let _res = ext_marketplace::return_service(
                    dispute.service_id,
                    &self.marketplace_account_id, NO_DEPOSIT, BASE_GAS)
                .then(ext_self::on_return_service(
                    dispute.service_id,
                    &env::current_account_id(), NO_DEPOSIT, BASE_GAS)
                );
            }
        }

        self.disputes.insert(&dispute_id, &dispute);

        return dispute;
    }

    //////////////////////////////////////
    ///         Metodos GET            ///
    //////////////////////////////////////
        
    pub fn get_dispute_status(&mut self, dispute_id: DisputeId) -> Dispute {
        self.update_dispute_status(dispute_id)
    }

    pub fn get_dispute(&self, dispute_id: DisputeId) -> Dispute {
        let dispute = expect_value_found(self.disputes.get(&dispute_id), 
        "Dispute not found".as_bytes());
        dispute
    }

    pub fn get_total_disputes(&self) -> u64 {
        self.disputes_counter
    }

    // pub fn get_admins(&self) -> vec!() {
    //     self.admins
    // }

    

    //////////////////////////////////////
    ///      Funciones internas        ///
    //////////////////////////////////////
    
    // fn assert_owner(&self, account: &AccountId) {
    //     if *account != self.owner {
    //         env::panic(b"Isn't the owner");
    //     }
    // }

    // fn assert_admin(&self, account: &AccountId) {
    //     if !self.admins.contains(&account) {
    //         env::panic(b"Isn't a Admin");
    //     }
    // }
    
    //////////////////////////////////////
    /// Llamados a los demás contratos ///
    //////////////////////////////////////

    /// Verificar datos de la disputa desde el contrato del marketplace
    /// 
    pub fn on_validate_dispute(&mut self) {
        if env::predecessor_account_id() != env::current_account_id() {
            env::panic(b"only the contract can call its function")
        }
        assert_eq!(
            env::promise_results_count(),
            1,
            "Contract expected a result on the callback"
        );
        match env::promise_result(0) {
            PromiseResult::Successful(_data) => {
                env::log(b"Dispute created");
            },
            PromiseResult::Failed => env::panic(b"Callback faild"),
            PromiseResult::NotReady => env::panic(b"Callback faild"),
        };
    }

    /// Retornar el servicio al profesional
    /// 
    pub fn on_return_service(_service_id: u64) {
        if env::predecessor_account_id() != env::current_account_id() {
            env::panic(b"only the contract can call its function")
        }

        assert_eq!(
            env::promise_results_count(),
            1,
            "Contract expected a result on the callback"
        );
        match env::promise_result(0) {
            PromiseResult::Successful(_data) => {
                env::log(b"Token devuelto :)");
            },
            PromiseResult::Failed => env::panic(b"Callback faild"),
            PromiseResult::NotReady => env::panic(b"Callback faild"),
        };
    }
}

#[ext_contract(ext_marketplace)]
pub trait Marketplace {
    fn validate_dispute(applicant: AccountId, accused: AccountId, service_id: u64, jugdes: u8, exclude: Vec<ValidAccountId>);
    fn return_service(service_id: u64);
}
#[ext_contract(ext_self)]
pub trait ExtSelf {
    fn on_validate_dispute(applicant: AccountId, accused: AccountId, service_id: u64, proves: String);
    fn on_return_service(service_id: u64);
}
#[ext_contract(ext_ft)]
pub trait ExtFT {
    fn increase_allowance(account: AccountId);
    fn decrease_allowance(account: AccountId);
}

fn expect_value_found<T>(option: Option<T>, message: &[u8]) -> T {
    option.unwrap_or_else(|| env::panic(message))
}

// pub(crate) fn string_to_valid_account_id(account_id: &String) -> ValidAccountId{
//     return ValidAccountId::try_from((*account_id).to_string()).unwrap();
// }

// pub(crate) fn unique_prefix(account_id: &AccountId) -> Vec<u8> {
//     let mut prefix = Vec::with_capacity(33);
//     prefix.push(b'o');
//     prefix.extend(env::sha256(account_id.as_bytes()));
//     return prefix
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use near_sdk::test_utils::{VMContextBuilder, accounts};
//     use near_sdk::MockedBlockchain;
//     use near_sdk::{testing_env, VMContext};

//     fn get_context(is_view: bool) -> VMContext {
//         VMContextBuilder::new()
//             .signer_account_id(accounts(1))
//             .predecessor_account_id(accounts(2))
//             .attached_deposit(100000000000000000)
//             .is_view(is_view)
//             .build()
//     }

//     fn get_account(id: usize) -> String {
//         return accounts(id).to_string()
//     }

//     #[test]
//     fn test1() {
//         let contract_account = "mediator.near";
//         let applicant = get_account(0);
//         let accused = get_account(1);
//         let judges = [get_account(2), get_account(3)];

//         let mut context = get_context(false);
//         context.attached_deposit = 58700000000000000000000;
//         context.epoch_height = 0;
//         context.predecessor_account_id = applicant.clone();
//         context.block_timestamp = 1640283546;
//         context.current_account_id = contract_account.to_string();
//         testing_env!(context);

//         let mut contract = Mediator::new("marketplace.near".to_string());
//         let mut dispute = contract.new_dispute_test(2, string_to_valid_account_id(&"employer".to_string()), "Prueba en markdown".to_string());

//         let mut context = get_context(false);
//         context.attached_deposit = 58700000000000000000000;
//         context.block_timestamp = 1640283546 + ONE_DAY;
//         context.epoch_height = 0;
//         context.predecessor_account_id = judges[0].clone();
//         context.current_account_id = contract_account.to_string();
//         testing_env!(context);
//         contract.add_judge_test(dispute.id.clone());

//         let mut context = get_context(false);
//         context.attached_deposit = 58700000000000000000000;
//         context.epoch_height = 0;
//         context.block_timestamp = 1640283546 + (ONE_DAY * 2);
//         context.predecessor_account_id = judges[1].clone();
//         context.current_account_id = contract_account.to_string();
//         testing_env!(context);
//         contract.add_judge_test(dispute.id.clone());

//         let mut context = get_context(false);
//         context.attached_deposit = 58700000000000000000000;
//         context.epoch_height = 0;
//         context.block_timestamp = 1640283546 + (ONE_DAY * 2);
//         context.predecessor_account_id = accused.clone();
//         context.current_account_id = contract_account.to_string();
//         testing_env!(context);
//         contract.add_accused_proves(dispute.id.clone(), "Markdown accused proves".to_string());

//         let max_epochs = 26;
//         let mut judges_votes = 0;
//         for i in 2..max_epochs {
//             let mut context = get_context(false);
//             if dispute.dispute_status == DisputeStatus::Resolving && judges_votes < 2{
//                 context.predecessor_account_id = judges[judges_votes].clone();
//                 contract.vote(dispute.id.clone(), true); //judges_votes != 0
//                 judges_votes += 1;
//             }
//             else {
//                 context.predecessor_account_id = applicant.clone();
//             }
//             context.attached_deposit = 58700000000000000000000;
//             context.epoch_height = i;
//             context.current_account_id = contract_account.to_string();
//             context.block_timestamp = 1640283546 + (ONE_DAY * i);
//             testing_env!(context.clone());
//             dispute = contract.update_dispute_status(dispute.id.clone());

//             println!("Epoca: {}, estatus: {:#?}, {:?}", context.block_timestamp, dispute.dispute_status, dispute.votes);

//         }
//         let winner = dispute.winner.expect("Debe haber un ganador");

//         println!("");
//         println!("The winner is {:?}", winner);
//     }
// }