import React, { useEffect, useRef, useState } from "react";
import {
  fireStore,
  auth,
  singInFirebaseAnonymously,
  firebaseApp,
} from "../firebase";
import { useAuthState } from "react-firebase-hooks/auth";
import { useCollectionData } from "react-firebase-hooks/firestore";
import {
  collection,
  query,
  addDoc,
  setDoc,
  doc,
  getDoc,
  orderBy,
  limit,
  Timestamp,
} from "firebase/firestore";

export default function Chat({ service }) {
  const salasRef = doc(
    fireStore,
    "salas",
    `${service.id}&${service.creator_id}&${service.actual_owner}`
  );
  const messagesRef = collection(
    fireStore,
    `salas/${service.id}&${service.creator_id}&${service.actual_owner}/messages`
  );
  const q = query(messagesRef, orderBy("createdAt"), limit(25));

  const [newMessage, setNewMessage] = useState("");
  const [loading, setLoading] = useState(true);
  const dummy = useRef();
  const [user] = useAuthState(auth);
  const [messages] = useCollectionData(q, { idField: "id" });

  useEffect(async () => {
    console.log(service);
    singInFirebaseAnonymously();

    const salasDoc = await getDoc(salasRef);
    if (!salasDoc.exists()) {
      await setDoc(salasRef, {
        members: [service.creator_id, service.actual_owner],
      });
    }

    setLoading(false);
  }, []);

  // useEffect(() => {
  //   if (dummy.current) {
  //     dummy.current.scrollIntoView({ behavior: "smooth", block: "center" });
  //   }
  // }, [dummy.current]);

  const handleOnChange = (e) => {
    setNewMessage(e.target.value);
  };

  const handleOnSubmit = async (e) => {
    e.preventDefault();
    setNewMessage("");

    const { uid } = auth.currentUser;
    await addDoc(messagesRef, {
      msg: newMessage,
      createdAt: Timestamp.fromDate(new Date()),
      uid,
    });

    if (dummy.current) {
      // dummy.current.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  };

  const dateToString = (date) => {
    let d = new Timestamp(date.seconds, date.nanoseconds).toDate();
    return d.toLocaleDateString();
  };

  return (
    <div className="container mx-auto">
      <div className=" border rounded">
        <div>
          <div className="w-full">
            {loading ? (
              <div className="flex justify-center m-8">
                <svg className="spinner-normal" viewBox="0 0 50 50">
                  <circle
                    className="path !stroke-[#27C0EF]"
                    cx="25"
                    cy="25"
                    r="20"
                    fill="none"
                    strokeWidth="5"
                  ></circle>
                </svg>
              </div>
            ) : (
              <>
                <div className="relative w-full p-6 overflow-y-auto h-[32rem]">
                  <ul className="space-y-2">
                    {messages.map((v, i) => {
                      return (
                        <li
                          key={i}
                          className={
                            v.uid == auth.currentUser.uid
                              ? "flex justify-end"
                              : "flex justify-start"
                          }
                        >
                          <div
                            className={
                              v.uid == auth.currentUser.uid
                                ? "relative max-w-xl px-4 pt-2 text-white bg-[#27C0EF] rounded shadow"
                                : "relative max-w-xl px-4 pt-2 text-gray-700 rounded shadow"
                            }
                          >
                            <span className="block whitespace-pre-wrap">
                              {v.msg}
                            </span>
                            <span className="block my-2 text-xs">
                              {dateToString(v.createdAt)}
                            </span>
                          </div>
                          {i + 1 == messages.length ? (
                            <div id="targetElement" ref={dummy}></div>
                          ) : (
                            <></>
                          )}
                        </li>
                      );
                    })}
                  </ul>
                </div>

                <form
                  onSubmit={handleOnSubmit}
                  className="flex items-center justify-between w-full p-3 border-t border-gray-300"
                >
                  <textarea
                    placeholder="Mensaje"
                    className="block w-full max-h-40 min-h-16 py-2 pl-4 mx-3 bg-gray-100 rounded-md outline-none focus:text-gray-700"
                    name="message"
                    value={newMessage}
                    onChange={handleOnChange}
                    required
                  />
                  <button type="submit" disabled={!newMessage}>
                    <svg
                      className="w-5 h-5 text-[#27C0EF] origin-center transform rotate-90"
                      xmlns="http://www.w3.org/2000/svg"
                      viewBox="0 0 20 20"
                      fill="currentColor"
                    >
                      <path d="M10.894 2.553a1 1 0 00-1.788 0l-7 14a1 1 0 001.169 1.409l5-1.429A1 1 0 009 15.571V11a1 1 0 112 0v4.571a1 1 0 00.725.962l5 1.428a1 1 0 001.17-1.408l-7-14z" />
                    </svg>
                  </button>
                </form>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
