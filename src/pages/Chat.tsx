import { useEffect, useState } from "react";
import { ChatEvent } from "types/events";
import { listen } from "@tauri-apps/api/event";
import TopBar from "components/elements/topbar";
import EventLogModal from "components/elements/eventLog";
import Messages from "components/elements/messages";
import { ChatMessage, PeerInfo } from "types";
import { getMessageLog, getNodeId, getPeers } from "services/ipc";

export function ChatPage() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [eventLog, setEventLog] = useState<ChatEvent[]>([]);
  const [neighbours, setNeighbours] = useState<Map<string, PeerInfo>>(
    new Map()
  );
  const [openLog, setOpenLog] = useState<boolean>(false);
  const [myNodeId, setMyNodeId] = useState<string | undefined>();
  useEffect(() => {
    getNodeId().then((id) => {
      setMyNodeId(id);
      getMessageLog().then((messages) => {
        setMessages(messages);
      });
      getPeers().then((peers) => {
        const newNeighbours = new Map();
        peers.forEach((peer) => {
          if (peer.id !== id) {
            newNeighbours.set(peer.id, peer);
          }
        });
        setNeighbours(newNeighbours);
      });
    });
  }, []);

  useEffect(() => {
    const eventsRef = listen<ChatEvent>("chat-event", async (event) => {
      setEventLog((eventLog) => [...eventLog, event.payload]);
      console.log(event);
      // any time anything changes, update all data.
      getMessageLog().then((messages) => {
        setMessages(messages);
      });
      getPeers().then((peers) => {
        const newNeighbours = new Map();
        peers.forEach((peer) => {
          if (peer.id !== myNodeId) {
            newNeighbours.set(peer.id, peer);
          }
        });
        setNeighbours(newNeighbours);
      });
      // if (event.payload.type === "newMessage") {
      //   const message = event.payload.message;
      //   setMessages((messages) => [...messages, message]);
      // } else if (event.payload.type === "peerUpdate") {
      //   const peer = event.payload.info;
      //   setNeighbours((neighbours) => {
      //     if (peer.id === myNodeId) return neighbours;
      //     if (!neighbours.has(peer.id)) {
      //       notify(`New peer joined ${peer.nickname}`);
      //     }
      //     const newNeighbours = new Map(neighbours); // react needs us to make a new object here
      //     newNeighbours.set(peer.id, peer);
      //     return newNeighbours;
      //   });
      // } else if (event.payload.type === "contentReady") {
      //   getMessageLog().then((messages) => {
      //     setMessages(messages);
      //   });
      //   getPeers().then((peers) => {
      //     const newNeighbours = new Map();
      //     peers.forEach((peer) => {
      //       if (peer.id === myNodeId) return;
      //       newNeighbours.set(peer.id, peer);
      //     });
      //     setNeighbours(newNeighbours);
      //   });
      // }
    });
    return () => {
      eventsRef.then((drop) => drop());
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
