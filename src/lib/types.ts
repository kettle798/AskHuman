export interface AskRequest {
  id: string;
  isMarkdown: boolean;
  questions: Question[];
}

export interface Question {
  message: string;
  predefinedOptions: string[];
  files: FileAttachment[];
}

export interface FileAttachment {
  path: string;
  name: string;
  size: number;
  isImage: boolean;
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
  alwaysOnTop: boolean;
  sourceName: string;
}

export interface QuestionAnswer {
  selectedOptions: string[];
  userInput: string;
  images: ImageAttachment[];
  files: string[];
}

export interface PopupSubmission {
  answers: QuestionAnswer[];
}

export interface GeneralConfig {
  theme: ThemeMode;
  alwaysOnTop: boolean;
}

export interface PopupChannelConfig {
  enabled: boolean;
  width: number;
  height: number;
  rememberSize: boolean;
}

export interface TelegramChannelConfig {
  enabled: boolean;
  botToken: string;
  chatId: string;
  apiBaseUrl: string;
}

export interface ChannelsConfig {
  popup: PopupChannelConfig;
  telegram: TelegramChannelConfig;
}

export interface AppConfig {
  general: GeneralConfig;
  channels: ChannelsConfig;
}

export interface HookStatus {
  installed: boolean;
  hooksJsonExists: boolean;
  supported: boolean;
}

export interface TelegramTestArgs {
  botToken: string;
  chatId: string;
  apiBaseUrl: string;
}
