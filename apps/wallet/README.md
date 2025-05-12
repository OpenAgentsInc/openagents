# OpenAgents Wallet

A self-custodial Bitcoin wallet with Lightning Network support built with React, TypeScript, and Vite.

## Features

- **Self-Custodial**: Maintain complete control of your funds with locally stored seed phrases
- **Lightning Network Support**: Send and receive Bitcoin over the Lightning Network
- **Secure Wallet Creation**: Generate new wallets with BIP39 seed phrases
- **Wallet Restoration**: Restore existing wallets using seed phrases
- **Multi-Screen Flow**: Intuitive user interface for wallet management
- **Dark Mode Support**: Choose between light and dark themes

## Technology Stack

- **Frontend**: React 19 with TypeScript
- **UI Components**: shadcn/ui for consistent, accessible UI
- **Styling**: Tailwind CSS
- **Bitcoin Integration**: Breez SDK Liquid for Lightning Network support
- **Build Tools**: Vite for fast development and optimized production builds

## Getting Started

1. Clone the repository
2. Install dependencies:
   ```bash
   cd apps/wallet
   yarn install
   ```
3. Set up environment variables:
   Create a `.env` file with:
   ```
   VITE_BREEZ_API_KEY=your_breez_api_key_here
   ```

4. Run the development server:
   ```bash
   yarn dev
   ```

## Scripts

- `yarn dev`: Start the development server
- `yarn build`: Build for production
- `yarn t`: Run TypeScript checks
- `yarn lint`: Run ESLint
- `yarn preview`: Preview the production build

## Contributing

This project is part of the OpenAgents ecosystem. Contributions are welcome!

## Security

This wallet is 100% self-custodial. Users are responsible for securing their seed phrases. Lost seed phrases cannot be recovered.

## License

Open source under the MIT license.