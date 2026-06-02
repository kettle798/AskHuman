export interface AskRequest {
  id: string;
  message: string;
  predefinedOptions: string[];
  isMarkdown: boolean;
}

export interface ImageAttachment {
  data: string;
  mediaType: string;
  filename?: string | null;
}

export type ThemeMode = "system" | "light" | "dark";

export interface PopupInit {
  request: AskRequest;
  theme: ThemeMode;
}

export interface PopupSubmission {
  selectedOptions: string[];
  userInput: string;
  images: ImageAttachment[];
}
