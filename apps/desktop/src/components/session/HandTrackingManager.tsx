import React from 'react';
import HandTracking from '../hands/HandTracking';

interface HandTrackingManagerProps {
  isActive: boolean;
  onHandDataUpdate: (data: any) => void;
}

export const HandTrackingManager: React.FC<HandTrackingManagerProps> = ({
  isActive,
  onHandDataUpdate,
}) => {
  return (
    <HandTracking
      showHandTracking={isActive}
      setShowHandTracking={() => {}}
      onHandDataUpdate={onHandDataUpdate}
    />
  );
};