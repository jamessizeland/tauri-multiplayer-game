export interface ChatMessage {
  sender: string;
  content: string;
  nickname: string;
  timestamp: number;
}

export interface PeerInfo {
  id: string;
  nickname: string;
  status: PeerStatus;
  ready: boolean;
}

export type PeerStatus = "Online" | "Offline" | "Unknown";
