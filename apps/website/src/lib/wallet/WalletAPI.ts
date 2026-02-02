import type {
  Config,
  GetInfoResponse,
  PrepareSendPaymentRequest,
  PrepareSendPaymentResponse,
  ReceivePaymentRequest,
  ReceivePaymentResponse,
  SendPaymentRequest,
  SendPaymentResponse,
  Payment,
  SdkEvent,
  InputType,
  LightningAddressInfo,
  PrepareLnurlPayRequest,
  PrepareLnurlPayResponse,
  LnurlPayRequest,
  LnurlPayResponse,
  DepositInfo,
  Fee,
  UserSettings,
  UpdateUserSettingsRequest,
  FiatCurrency,
  Rate,
} from "@breeztech/breez-sdk-spark";

export interface WalletAPI {
  initWallet: (mnemonic: string, config: Config) => Promise<void>;
  disconnect: () => Promise<void>;
  connected: () => boolean;

  parseInput: (input: string) => Promise<InputType>;
  prepareLnurlPay: (params: PrepareLnurlPayRequest) => Promise<PrepareLnurlPayResponse>;
  lnurlPay: (params: LnurlPayRequest) => Promise<LnurlPayResponse>;
  prepareSendPayment: (params: PrepareSendPaymentRequest) => Promise<PrepareSendPaymentResponse>;
  sendPayment: (params: SendPaymentRequest) => Promise<SendPaymentResponse>;
  receivePayment: (params: ReceivePaymentRequest) => Promise<ReceivePaymentResponse>;
  unclaimedDeposits: () => Promise<DepositInfo[]>;
  claimDeposit: (txid: string, vout: number, maxFee: Fee) => Promise<void>;
  refundDeposit: (txid: string, vout: number, destinationAddress: string, fee: Fee) => Promise<void>;

  getWalletInfo: () => Promise<GetInfoResponse | null>;
  getTransactions: () => Promise<Payment[]>;

  addEventListener: (callback: (event: SdkEvent) => void) => Promise<string>;
  removeEventListener: (listenerId: string) => Promise<void>;

  saveMnemonic: (mnemonic: string) => void;
  getSavedMnemonic: () => string | null;
  clearMnemonic: () => void;

  getLightningAddress: () => Promise<LightningAddressInfo | null>;
  checkLightningAddressAvailable: (username: string) => Promise<boolean>;
  registerLightningAddress: (username: string, description: string) => Promise<void>;
  deleteLightningAddress: () => Promise<void>;

  getUserSettings: () => Promise<UserSettings>;
  setUserSettings: (settings: UpdateUserSettingsRequest) => Promise<void>;

  listFiatCurrencies: () => Promise<FiatCurrency[]>;
  listFiatRates: () => Promise<Rate[]>;

  getSdkLogs: () => string;
  getAppLogs: () => string;
  getAllLogs: () => string;
  getAllLogsAsZip: () => Promise<Blob>;
  canShareFiles: () => boolean;
  shareOrDownloadLogs: () => Promise<void>;

  initLogSession: () => Promise<void>;
  endLogSession: () => Promise<void>;
}
