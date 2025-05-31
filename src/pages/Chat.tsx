import { useEffect, useState } from "react";
import { ChatEvent } from "types/events";
import { listen } from "@tauri-apps/api/event";
import TopBar from "components/elements/topbar";
import EventLogModal from "components/elements/eventLog";
import Messages from "components/elements/messages";
import { ChatMessage, PeerInfo } from "types";

export function ChatPage() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [eventLog, setEventLog] = useState<ChatEvent[]>([]);
  const [neighbours, setNeighbours] = useState<PeerInfo[]>([]);
  const [openLog, setOpenLog] = useState<boolean>(false);

  useEffect(() => {
    const updatePeersRef = listen<PeerInfo[]>("peer-update", async (event) => {
      console.log(event.payload);
      setNeighbours(event.payload);
    });
    const messagesRef = listen<ChatMessage>("new-message", async (event) => {
      console.log(event.payload);
      setMessages((messages) => [...messages, event.payload]);
    });

    const eventsRef = listen<ChatEvent>("chat-event", async (event) => {
      console.log(event);
      setEventLog((eventLog) => [...eventLog, event.payload]);
    });
    return () => {
      updatePeersRef.then((drop) => drop());
      eventsRef.then((drop) => drop());
      messagesRef.then((drop) => drop());
    };
  }, []);

  return (
    <div className="flex flex-col items-center h-screen w-screen space-y-2">
      <TopBar openEventLog={() => setOpenLog(true)} neighbours={neighbours} />
      <EventLogModal
        eventLog={eventLog}
        isOpen={openLog}
        onClose={() => setOpenLog(false)}
      />
      <Messages messages={messages} />
    </div>
  );
}
