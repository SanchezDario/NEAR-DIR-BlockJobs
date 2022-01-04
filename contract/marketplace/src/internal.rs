use crate::*;
/// Price per 1 byte of storage from mainnet config after `1.18.0` release and protocol version `42`.
/// It's 10 times lower than the genesis price.

// Esto esta en yocto near
pub(crate) const YOCTO_NEAR: u128 = 1000000000000000000000000;
pub(crate) const STORAGE_PRICE_PER_BYTE: Balance = 10_000_000_000_000_000_000;

pub(crate) fn string_to_valid_account_id(account_id: &String) -> ValidAccountId{
    return ValidAccountId::try_from((*account_id).to_string()).unwrap();
}

pub(crate) fn unique_prefix(account_id: &AccountId) -> Vec<u8> {
    let mut prefix = Vec::with_capacity(33);
    prefix.push(b'o');
    prefix.extend(env::sha256(account_id.as_bytes()));
    prefix
}

pub(crate) fn deposit_refund(storage_used: u64) {
    let required_cost = STORAGE_PRICE_PER_BYTE * Balance::from(storage_used);
    let attached_deposit = env::attached_deposit();

    assert!(
        required_cost <= attached_deposit,
        "Requires to attach {:.1$} NEAR services to cover storage",required_cost as f64 / YOCTO_NEAR as f64, 3 // la presicion de decimales
    );

    let refund = attached_deposit - required_cost;
    if refund > 0 {
        Promise::new(env::predecessor_account_id()).transfer(refund);
    }
}

pub(crate) fn deposit_refund_to(storage_used: u64, to: AccountId) {
    let required_cost = env::storage_byte_cost() * Balance::from(storage_used);
    let attached_deposit = env::attached_deposit();

    assert!(
        required_cost <= attached_deposit,
        "Requires to attach {:.1$} NEAR services to cover storage",required_cost as f64 / YOCTO_NEAR as f64, 3 // la presicion de decimales
    );

    let refund = attached_deposit - required_cost;
    if refund > 0 {
        Promise::new(to).transfer(refund);
    }
}

// pub(crate) fn bytes_for_approved_account_id(account_id: &AccountId) -> u64 {
//     // The extra 4 bytes are coming from Borsh serialization to store the length of the string.
//     account_id.len() as u64 + 4
// }

// pub(crate) fn refund_approved_account_ids(
//     account_id: AccountId,
//     approved_account_ids: &HashSet<AccountId>,
// ) -> Promise {
//     let storage_released: u64 = approved_account_ids
//         .iter()
//         .map(bytes_for_approved_account_id)
//         .sum();
//     Promise::new(account_id).transfer(Balance::from(storage_released) * STORAGE_PRICE_PER_BYTE)
// }

impl Marketplace {

    pub(crate) fn get_users(&self, from_index: u64, limit: u64) -> Vec<(AccountId, User)> {
        let keys = self.users.keys_as_vector();
        let values = self.users.values_as_vector();
        (from_index..std::cmp::min(from_index + limit, self.users.len()))
            .map(|index| (keys.get(index).unwrap(), values.get(index).unwrap()))
            .collect()
    }

    pub(crate) fn add_service(&mut self, service_id: &u64, account_id: &String) {
        let mut services_set = self
            .services_by_account
            .get(account_id)
            .unwrap_or_else(|| UnorderedSet::new(unique_prefix(&account_id)));
        services_set.insert(service_id);
        self.services_by_account.insert(account_id, &services_set);
    }

    pub(crate) fn delete_service(&mut self, service_id: &u64, account_id: &String) {
        let mut services_set = expect_value_found(self.services_by_account.get(account_id), "Service should be owned by the sender".as_bytes());
        services_set.remove(service_id);
        self.services_by_account.insert(&account_id, &services_set);
    }

    #[allow(unused_variables)]
    pub(crate) fn get_random_users_account_by_role_jugde(&self, amount: u8, exclude: Vec<ValidAccountId>) -> Vec<AccountId> {
        if amount > 10 {
            env::panic(b"No se puede pedir mas de 10");
        }
        let users = self.get_users_by_role(UserRoles::Jugde, 0, amount.into());
        if amount as usize > users.len() {
            env::panic(b"La cantidad pedida es mayor a la existente");
        }

        let sample = users.choose(&mut rand::thread_rng());
        return sample
            .iter()
            .filter(|x| exclude.contains(&string_to_valid_account_id(&x.account_id)))
            .map(|x| x.account_id.clone())
            .collect();
    }

    pub(crate) fn measure_min_service_storage_cost(&mut self) {
        let initial_storage_usage = env::storage_usage();
        let tmp_account_id = "a".repeat(64);
        let u = UnorderedSet::new(unique_prefix(&tmp_account_id));
        self.services_by_account.insert(&tmp_account_id, &u);

        let services_by_account_entry_in_bytes = env::storage_usage() - initial_storage_usage;
        let owner_id_extra_cost_in_bytes = (tmp_account_id.len() - self.contract_owner.len()) as u64;

        self.extra_storage_in_bytes_per_service =
            services_by_account_entry_in_bytes + owner_id_extra_cost_in_bytes;

        self.services_by_account.remove(&tmp_account_id);
    }

    pub(crate) fn update_user_mints(&mut self, quantity: u16) -> User {
        let sender = env::predecessor_account_id();
        let mut user = expect_value_found(self.users.get(&sender), "Before mint a nft, create an user".as_bytes());
        
        if user.mints + quantity > USER_MINT_LIMIT {
            env::panic(format!("Exceeded user mint limit {}", USER_MINT_LIMIT).as_bytes());
        }
        user.mints += quantity;

        self.users.insert(&sender, &user);

        return user
    }

    /********* ASSERTS  ***********/

    /// Verificar que sea el admin
    pub(crate) fn assert_admin(&self, account_id: &AccountId) {
        if *account_id != self.contract_owner {
            env::panic("Must be owner_id how call its function".as_bytes())
        }
    }

    pub(crate) fn assert_service_exists(&self, service_id: &u64) {
        if *service_id > self.total_services {
            env::panic(b"The indicated service doesn't exist")
        }
    }

    // pub(crate) fn internal_remove_service_from_owner(
    //     &mut self,
    //     account_id: &AccountId,
    //     service_id: &ServiceId,
    // ) {
    //     let mut services_set = self
    //         .services_by_account
    //         .get(account_id)
    //         .expect("Service should be owned by the sender");
    //     services_set.remove(service_id);
    //     if services_set.is_empty() {
    //         self.services_by_account.remove(account_id);
    //     } else {
    //         self.services_by_account.insert(account_id, &services_set);
    //     }
    // }

    // pub(crate) fn internal_transfer(
    //     &mut self,
    //     sender_id: &AccountId,
    //     receiver_id: &AccountId,
    //     service_id: &ServiceId,
    //     enforce_approval_id: Option<u64>,
    //     memo: Option<String>,
    // ) -> (AccountId, HashSet<AccountId>) {
    //     let Service {
    //         owner_id,
    //         metadata,
    //         employer_account_ids,
    //         employer_id,
    //     } = self.service_by_id.get(service_id).expect("Service not found");
    //     if sender_id != &owner_id && !employer_account_ids.contains(sender_id) {
    //         env::panic(b"Unauthorized");
    //     }

    //     if let Some(enforce_approval_id) = enforce_approval_id {
    //         assert_eq!(
    //             employer_id,
    //             enforce_approval_id,
    //             "The service approval_id is different from provided"
    //         );
    //     }

    //     assert_ne!(
    //         &owner_id, receiver_id,
    //         "The service owner and the receiver should be different"
    //     );

    //     env::log(
    //         format!(
    //             "Transfer {} from @{} to @{}",
    //             service_id, &owner_id, receiver_id
    //         )
    //         .as_bytes(),
    //     );

    //     self.internal_remove_service_from_owner(&owner_id, service_id);
    //     self.internal_add_service_to_owner(receiver_id, service_id);

    //     let service = Service {
    //         owner_id: receiver_id.clone(),
    //         metadata,
    //         employer_account_ids: Default::default(),
    //         employer_id: employer_id + 1,
    //     };
    //     self.service_by_id.insert(service_id, &service);

    //     if let Some(memo) = memo {
    //         env::log(format!("Memo: {}", memo).as_bytes());
    //     }

    //     (owner_id, employer_account_ids)
    // }

    // #[private]
    // fn string_to_json(&self, service_id: ServiceId) -> Category {
    //     let example = Category {
    //         category: "Programmer".to_string(),
    //         subcategory: "Backend".to_string(),
    //         areas: "Python, SQL".to_string()
    //     };
    //     let serialized = serde_json::to_string(&example).unwrap();

    //     let string = format!("String: {}", &serialized);
    //     env::log(string.as_bytes());

    // // pub fn string_to_json(&self, service_id: ServiceId) -> Category {
    // pub fn string_to_json(&self) -> Category {
    //     let example = Category {
    //         category: "Programmer".to_string(),
    //         subcategory: "Backend".to_string(),
    //         areas: "Python, SQL".to_string()
    //     };
    //     let serialized = serde_json::to_string(&example).unwrap();

    //     let string = format!("String: {}", &serialized);
    //     env::log(string.as_bytes());

    //     let deserialized: Category = serde_json::from_str(&serialized).unwrap();
    //     deserialized
    // }
]
