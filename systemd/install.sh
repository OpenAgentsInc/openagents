#!/bin/bash
# Install autopilot systemd services and timers

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Installing Autopilot Systemd Services${NC}"
echo

# Check if systemd is available
if ! command -v systemctl &> /dev/null; then
    echo -e "${RED}Error: systemd not found${NC}"
    exit 1
fi

# Create user systemd directory if it doesn't exist
mkdir -p ~/.config/systemd/user

# Copy service and timer files
echo -e "${YELLOW}Copying service files...${NC}"
cp autopilot-baseline-update.service ~/.config/systemd/user/
cp autopilot-baseline-update.timer ~/.config/systemd/user/

# Reload systemd
echo -e "${YELLOW}Reloading systemd daemon...${NC}"
systemctl --user daemon-reload

# Enable and start the timer
echo -e "${YELLOW}Enabling and starting baseline update timer...${NC}"
systemctl --user enable autopilot-baseline-update.timer
systemctl --user start autopilot-baseline-update.timer

echo
echo -e "${GREEN}✅ Installation complete!${NC}"
echo
echo "The baseline update timer is now active and will run every Monday at 00:00."
echo
echo "Useful commands:"
echo "  systemctl --user status autopilot-baseline-update.timer  # Check timer status"
echo "  systemctl --user list-timers                              # List all timers"
echo "  journalctl --user -u autopilot-baseline-update.service   # View service logs"
echo "  systemctl --user stop autopilot-baseline-update.timer    # Stop the timer"
echo
