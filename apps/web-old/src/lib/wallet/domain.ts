import type { InputType } from "@breeztech/breez-sdk-spark";

export type PaymentMethod = "lightning" | "spark" | "bitcoin";
export type ReceiveStep = "loading_limits" | "input" | "qr" | "loading";
export type PaymentStep = "input" | "amount" | "fee" | "confirm" | "processing" | "result";

export interface SendInput {
  rawInput: string;
  parsedInput: InputType;
}
