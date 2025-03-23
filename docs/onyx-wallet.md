# Onyx Wallet Integration

## Overview

The Onyx mobile app includes a built-in Bitcoin wallet powered by the Breez SDK (Liquid Implementation) to enable seamless payments within the OpenAgents ecosystem. This wallet serves as the foundation for our monetization layer, allowing users to pay for agent usage and contributors to receive payments for their contributions.

## Current Implementation

The current wallet implementation includes basic functionality:

- Main wallet screen with balance display
- Send Bitcoin functionality
- Receive Bitcoin functionality
- Transaction history
- Backup/restore wallet functionality

## Technical Stack

- Breez SDK - Nodeless (Liquid Implementation)
  - Self-custodial Lightning payments
  - No channel management required
  - Built-in fiat on-ramp
  - Multi-asset support (Bitcoin, USDT)
  - Minimum payment size: 1,000 sats
  - Static low fees

## Planned Additional Screens

### 1. Agent Payment Screen
- Quick payment interface for agent usage
- Pre-calculated costs based on expected usage
- Payment confirmation with detailed breakdown
- Option to top up if balance insufficient

### 2. Contributor Dashboard
- Overview of earnings from contributions
- Breakdown by contribution type:
  - Compute provision
  - Model usage
  - Data/content contribution
  - Agent skill creation
  - Referral rewards
- Withdrawal options
- Historical earnings data

### 3. Payment Settings
- Default payment preferences
- Auto-top-up settings
- Notification preferences for:
  - Low balance
  - Incoming payments
  - Successful payments
  - Failed payments

### 4. Fiat On-ramp Integration
- Multiple payment method support
- KYC flow if required
- Purchase history
- Conversion rate display
- Fee transparency

### 5. Asset Management
- Toggle between BTC and USDT
- Asset conversion interface
- Asset-specific transaction history
- Balance display in preferred currency

### 6. Security Settings
- Backup reminder system
- Recovery phrase verification
- Biometric authentication settings
- Transaction limits
- Pin code management

## Integration with Agent System

The wallet will be tightly integrated with the agent system:

1. **Automatic Payments**
   - Pay-per-use agent interactions
   - Subscription-based access
   - Automatic rewards for contributions

2. **Smart Notifications**
   - Balance alerts before agent usage
   - Payment confirmations
   - Earning notifications
   - Contribution reward alerts

3. **Usage Analytics**
   - Spending patterns
   - Most used agents
   - Contribution metrics
   - ROI calculations for contributors

## Security Considerations

- All private keys are held only by users
- Real-time state backup
- Multi-device support
- Optional biometric authentication
- Transparent fee structure
- Clear transaction history

## Future Enhancements

1. **Social Features**
   - In-app tipping for helpful contributions
   - Team wallet functionality
   - Social payments (split bills, group purchases)

2. **Advanced Analytics**
   - Detailed spending reports
   - Contribution analytics
   - Performance metrics for agent creators

3. **Integration Expansions**
   - Additional payment methods
   - More fiat currencies
   - Cross-platform synchronization

4. **Developer Tools**
   - Payment API access
   - Custom integration options
   - Webhook support

## Implementation Timeline

1. **Phase 1 - Core Functionality** (Current)
   - Basic send/receive
   - Balance display
   - Transaction history
   - Backup/restore

2. **Phase 2 - Agent Integration**
   - Agent payment screen
   - Contributor dashboard
   - Basic analytics

3. **Phase 3 - Enhanced Features**
   - Fiat on-ramp
   - Asset management
   - Advanced security settings

4. **Phase 4 - Advanced Integration**
   - Social features
   - Advanced analytics
   - Developer tools