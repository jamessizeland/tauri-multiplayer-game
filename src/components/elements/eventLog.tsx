import React, { useEffect, useRef } from "react";
import { GrClose } from "react-icons/gr";
import { ChatEvent } from "types/events";

interface EventLogModalProps {
  isOpen: boolean;
  onClose: () => void;
  eventLog: ChatEvent[];
}

const EventLogModal: React.FC<EventLogModalProps> = ({
  isOpen,
  onClose,
  eventLog,
}) => {
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialogNode = dialogRef.current;
    if (!dialogNode) return;

    if (isOpen) {
      if (!dialogNode.hasAttribute("open")) {
        dialogNode.showModal(); // Use showModal() for true modal behavior
      }
    } else {
      if (dialogNode.hasAttribute("open")) {
        dialogNode.close();
      }
    }
  }, [isOpen]);

  // Handles the dialog's native 'close' event (e.g., ESC key)
  useEffect(() => {
    const dialogNode = dialogRef.current;
    if (!dialogNode) return;

    const handleNativeClose = () => {
      if (isOpen) {
        // Only call onClose if the parent thinks it's open
        onClose(); // Sync parent state
      }
    };

    dialogNode.addEventListener("close", handleNativeClose);
    return () => {
      dialogNode.removeEventListener("close", handleNativeClose);
    };
  }, [isOpen, onClose]);

  return (
    <dialog ref={dialogRef} id="event-log" className="modal">
      <div className="modal-box flex justify-center flex-col items-center">
        <h3 className="text-lg font-semibold mb-2">Event Log</h3>
        <div className="h-64 w-full overflow-y-auto border border-gray-300 rounded-md p-2 bg-base-200">
          {eventLog.length === 0 && (
            <p className="text-gray-500">No events yet.</p>
          )}
          {eventLog
            .slice()
            .reverse()
            .map((event, index) => (
              <div
                key={index}
                className="p-1 border-b border-gray-400 text-sm w-full"
              >
                <RenderEvent event={event} />
              </div>
            ))}
        </div>
        <div className="modal-action flex justify-end">
          <button className="btn btn-accent" onClick={onClose}>
            Close <GrClose />
          </button>
        </div>
      </div>
    </dialog>
  );
};

export default EventLogModal;

const RenderEvent: React.FC<{ event: ChatEvent }> = ({ event }) => {
  const Card: React.FC<React.PropsWithChildren<{ title: string }>> = ({
    title,
    children,
  }) => (
    <div className="card card-compact bg-base-100 shadow-md my-1 w-full">
      <div className="card-body p-3">
        <h4 className="card-title text-sm font-semibold">{title}</h4>
        <div className="text-xs space-y-0.5">{children}</div>
      </div>
    </div>
  );

  const Property: React.FC<React.PropsWithChildren<{ label: string }>> = ({
    label,
    children,
  }) => (
    <div>
      <span className="font-medium">{label}: </span>
      <span className="opacity-80 break-all">{children}</span>
    </div>
  );

  switch (event.type) {
    case "syncFinished":
      return <Card title="Sync Finished"></Card>;
    case "contentReady":
      return <Card title="Content Ready"></Card>;
    case "pendingContentReady":
      return <Card title="Pending Content Ready"></Card>;
    case "disconnected":
      return <Card title="Disconnected"></Card>;
    case "peerUpdate":
      return (
        <Card title="Peer Info Updated">
          <Property label="Node ID">{event.info.id}</Property>
          <Property label="Nickname">{event.info.nickname}</Property>
          <Property label="Status">{event.info.status}</Property>
          <Property label="Ready">{event.info.ready}</Property>
        </Card>
      );
    case "newMessage":
      return (
        <Card title="Message Received">
          <Property label="From">{event.message.sender}</Property>
          <Property label="Nickname">{event.message.nickname}</Property>
          <Property label="Message">{event.message.content}</Property>
          <Property label="Timestamp">
            {new Date(event.message.timestamp / 1000).toLocaleString()}
          </Property>
        </Card>
      );
    case "neighborUp":
      return (
        <Card title="Neighbor Connected">
          <Property label="Node ID">{event.nodeId}</Property>
        </Card>
      );
    case "neighborDown":
      return (
        <Card title="Neighbor Disconnected">
          <Property label="Node ID">{event.nodeId}</Property>
        </Card>
      );
    case "errored":
      return (
        <Card title="Error Occurred">
          <Property label="Message">{event.message}</Property>
        </Card>
      );
    default:
      // This case should ideally not be reached if ChatEvent is a well-defined discriminated union
      // and all types are handled. TypeScript helps ensure this.
      // const _exhaustiveCheck: never = event; // Uncomment for exhaustive check
      return (
        <Card title="Unknown Event">
          <pre className="text-xs whitespace-pre-wrap break-all">
            {JSON.stringify(event, null, 2)}
          </pre>
        </Card>
      );
  }
};
