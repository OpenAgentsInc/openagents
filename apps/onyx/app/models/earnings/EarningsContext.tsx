import React, { createContext, useContext, useState, useEffect } from 'react';

interface Earning {
  id: string;
  amount: number; // in satoshis
  timestamp: number;
  category: 'compute' | 'plugin' | 'referral' | 'content';
  description: string;
}

type TimePeriod = 'week' | 'month' | 'year' | 'all';

interface EarningsContextType {
  earnings: Earning[];
  totalEarnings: number;
  addEarning: (earning: Omit<Earning, 'id' | 'timestamp'>) => void;
  clearEarnings: () => void;
  getEarningsForPeriod: (period: TimePeriod) => Earning[];
  getTotalForPeriod: (period: TimePeriod) => number;
  withdrawEarnings: () => number; // Withdraw all earnings and return amount withdrawn
}

const EarningsContext = createContext<EarningsContextType | undefined>(undefined);

// Generate a timestamp for a specific time in the past (days ago)
const getTimestampDaysAgo = (days: number) => Date.now() - (86400000 * days);

// Generate historical earnings data with timestamps spread across various time periods
const generateHistoricalEarnings = (): Earning[] => {
  const categories = ['compute', 'plugin', 'referral', 'content'] as const;
  const descriptions = [
    'MCP Server Usage',
    'Agent Plugin',
    'Referral Rewards',
    'Content Creation'
  ];
  
  const earnings: Earning[] = [];
  let id = 1;
  
  // Weekly data - last 7 days
  // Create multiple entries per day for the last week (higher frequency)
  for (let day = 0; day < 7; day++) {
    const entriesPerDay = Math.floor(Math.random() * 3) + 1; // 1-3 entries per day
    
    for (let entry = 0; entry < entriesPerDay; entry++) {
      const categoryIndex = Math.floor(Math.random() * categories.length);
      earnings.push({
        id: id.toString(),
        amount: Math.floor(Math.random() * 500) + 100, // 100-600 sats
        timestamp: getTimestampDaysAgo(day) - (entry * 3600000), // Spread entries within the day
        category: categories[categoryIndex],
        description: descriptions[categoryIndex],
      });
      id++;
    }
  }
  
  // Monthly data - days 8-30
  for (let day = 8; day < 30; day++) {
    // Less frequent entries for older days
    if (Math.random() < 0.4) { // 40% chance of entry per day
      const categoryIndex = Math.floor(Math.random() * categories.length);
      earnings.push({
        id: id.toString(),
        amount: Math.floor(Math.random() * 800) + 200, // 200-1000 sats
        timestamp: getTimestampDaysAgo(day),
        category: categories[categoryIndex],
        description: descriptions[categoryIndex],
      });
      id++;
    }
  }
  
  // Yearly data - days 31-365
  for (let day = 31; day < 365; day += 5) { // Every ~5 days
    if (Math.random() < 0.3) { // 30% chance to include each period
      const categoryIndex = Math.floor(Math.random() * categories.length);
      earnings.push({
        id: id.toString(),
        amount: Math.floor(Math.random() * 1500) + 500, // 500-2000 sats
        timestamp: getTimestampDaysAgo(day),
        category: categories[categoryIndex],
        description: descriptions[categoryIndex],
      });
      id++;
    }
  }
  
  // Sort by timestamp (newest first)
  return earnings.sort((a, b) => b.timestamp - a.timestamp);
};

// Initial demo earnings data
const initialEarnings: Earning[] = generateHistoricalEarnings();

export const EarningsProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [earnings, setEarnings] = useState<Earning[]>(initialEarnings);

  // Calculate total earnings
  const totalEarnings = earnings.reduce((sum, earning) => sum + earning.amount, 0);

  // Get timestamps for period boundaries
  const getTimestampForPeriod = (period: TimePeriod): number => {
    const now = Date.now();
    switch (period) {
      case 'week':
        return now - 7 * 24 * 60 * 60 * 1000; // 7 days ago
      case 'month':
        return now - 30 * 24 * 60 * 60 * 1000; // 30 days ago
      case 'year':
        return now - 365 * 24 * 60 * 60 * 1000; // 365 days ago
      case 'all':
      default:
        return 0; // All time (beginning of time)
    }
  };

  // Get earnings for a specific time period
  const getEarningsForPeriod = (period: TimePeriod): Earning[] => {
    const periodStartTimestamp = getTimestampForPeriod(period);
    return earnings.filter(earning => earning.timestamp >= periodStartTimestamp);
  };

  // Get total for a specific time period
  const getTotalForPeriod = (period: TimePeriod): number => {
    return getEarningsForPeriod(period).reduce((sum, earning) => sum + earning.amount, 0);
  };

  // Add a new earning
  const addEarning = (earning: Omit<Earning, 'id' | 'timestamp'>) => {
    const newEarning: Earning = {
      ...earning,
      id: Math.random().toString(36).substring(2, 9),
      timestamp: Date.now(),
    };

    setEarnings((prevEarnings) => [newEarning, ...prevEarnings]);
  };

  // Clear all earnings
  const clearEarnings = () => {
    setEarnings([]);
  };
  
  // Withdraw all earnings (clear them and return the total)
  const withdrawEarnings = () => {
    const amount = totalEarnings;
    setEarnings([]); // Clear earnings after withdrawal
    return amount;
  };

  const value = {
    earnings,
    totalEarnings,
    addEarning,
    clearEarnings,
    getEarningsForPeriod,
    getTotalForPeriod,
    withdrawEarnings,
  };

  return <EarningsContext.Provider value={value}>{children}</EarningsContext.Provider>;
};

export const useEarnings = () => {
  const context = useContext(EarningsContext);
  if (context === undefined) {
    throw new Error('useEarnings must be used within an EarningsProvider');
  }
  return context;
};
