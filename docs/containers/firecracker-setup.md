# Firecracker Setup Guide for Home Linux Desktop

This guide walks through setting up Firecracker on your home Linux desktop to test multiple microVM deployments. We'll create a working environment where you can launch hundreds of lightweight VMs to simulate high-density Psionic application hosting.

## Prerequisites

### System Requirements

- **Linux kernel**: 4.14+ (with KVM support)
- **CPU**: Intel VT-x or AMD-V virtualization support
- **RAM**: Minimum 8GB (16GB+ recommended for testing many VMs)
- **Storage**: 20GB free space for images and kernels
- **Architecture**: x86_64 (arm64 also supported but this guide focuses on x86_64)

### Check Virtualization Support

```bash
# Check CPU virtualization support
grep -E 'vmx|svm' /proc/cpuinfo > /dev/null && echo "Virtualization supported" || echo "No virtualization support"

# Check KVM module is loaded
lsmod | grep kvm

# If KVM not loaded, load it
sudo modprobe kvm
sudo modprobe kvm_intel  # For Intel CPUs
# OR
sudo modprobe kvm_amd    # For AMD CPUs
```

### Required Packages

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install -y curl git build-essential qemu-utils bridge-utils net-tools

# Fedora/RHEL
sudo dnf install -y curl git gcc make qemu-img bridge-utils net-tools

# Arch
sudo pacman -S curl git base-devel qemu bridge-utils net-tools
```

## Installing Firecracker

### Download Firecracker Binary

```bash
# Create directory structure
mkdir -p ~/firecracker/{bin,kernels,rootfs,configs,logs}
cd ~/firecracker

# Set latest version (check https://github.com/firecracker-microvm/firecracker/releases)
FIRECRACKER_VERSION="v1.5.0"
ARCH="x86_64"

# Download Firecracker binary
curl -L "https://github.com/firecracker-microvm/firecracker/releases/download/${FIRECRACKER_VERSION}/firecracker-${FIRECRACKER_VERSION}-${ARCH}.tgz" | tar -xz
mv release-${FIRECRACKER_VERSION}-${ARCH}/firecracker-${FIRECRACKER_VERSION}-${ARCH} bin/firecracker
mv release-${FIRECRACKER_VERSION}-${ARCH}/jailer-${FIRECRACKER_VERSION}-${ARCH} bin/jailer

# Make binaries executable
chmod +x bin/{firecracker,jailer}

# Clean up
rm -rf release-${FIRECRACKER_VERSION}-${ARCH}

# Verify installation
./bin/firecracker --version
```

### Set Up Access Permissions

```bash
# Add yourself to KVM group
sudo usermod -aG kvm $USER

# Create /dev/kvm if it doesn't exist
sudo mknod /dev/kvm c 10 232

# Set permissions
sudo chmod 666 /dev/kvm
sudo chown root:kvm /dev/kvm

# Log out and back in for group changes to take effect
# Or use: newgrp kvm
```

## Building a Minimal Kernel

### Download Pre-built Kernel (Recommended for Testing)

```bash
cd ~/firecracker/kernels

# Download minimal kernel optimized for Firecracker
wget https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/x86_64/kernels/vmlinux.bin

# Alternative: Download from Firecracker's CI artifacts
curl -fsSL -o vmlinux-5.10.bin https://github.com/firecracker-microvm/firecracker/releases/download/v1.5.0/vmlinux-5.10.204
```

### Build Custom Kernel (Optional)

```bash
# Download kernel source
cd /tmp
wget https://cdn.kernel.org/pub/linux/kernel/v5.x/linux-5.10.204.tar.xz
tar -xf linux-5.10.204.tar.xz
cd linux-5.10.204

# Download Firecracker's recommended config
curl -o .config https://raw.githubusercontent.com/firecracker-microvm/firecracker/main/resources/guest_configs/microvm-kernel-x86_64-5.10.config

# Build kernel (this takes 10-30 minutes)
make olddefconfig
make -j$(nproc) vmlinux

# Copy to firecracker directory
cp vmlinux ~/firecracker/kernels/vmlinux-custom
```

## Creating Root Filesystems

### Option 1: Alpine Linux (Minimal ~50MB)

```bash
cd ~/firecracker/rootfs

# Create filesystem image
dd if=/dev/zero of=alpine.ext4 bs=1M count=128
mkfs.ext4 alpine.ext4

# Mount and set up Alpine
mkdir -p /tmp/alpine-rootfs
sudo mount alpine.ext4 /tmp/alpine-rootfs

# Download Alpine mini root filesystem
wget https://dl-cdn.alpinelinux.org/alpine/v3.19/releases/x86_64/alpine-minirootfs-3.19.0-x86_64.tar.gz
sudo tar -xzf alpine-minirootfs-3.19.0-x86_64.tar.gz -C /tmp/alpine-rootfs

# Configure Alpine for Firecracker
sudo tee /tmp/alpine-rootfs/etc/resolv.conf << EOF
nameserver 8.8.8.8
nameserver 8.8.4.4
EOF

# Set up init script
sudo tee /tmp/alpine-rootfs/etc/init.d/firecracker-setup << 'EOF'
#!/bin/sh
# Set up lo interface
ip link set lo up
# Mount essential filesystems
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev
# Start shell on console
exec /bin/sh
EOF

sudo chmod +x /tmp/alpine-rootfs/etc/init.d/firecracker-setup

# Configure inittab for console
sudo tee /tmp/alpine-rootfs/etc/inittab << EOF
::sysinit:/etc/init.d/firecracker-setup
::respawn:/sbin/getty -L 115200 ttyS0 vt100
EOF

# Unmount
sudo umount /tmp/alpine-rootfs
```

### Option 2: Ubuntu-based Rootfs (More Features ~200MB)

```bash
cd ~/firecracker/rootfs

# Create larger filesystem
dd if=/dev/zero of=ubuntu.ext4 bs=1M count=256
mkfs.ext4 ubuntu.ext4

# Mount
mkdir -p /tmp/ubuntu-rootfs
sudo mount ubuntu.ext4 /tmp/ubuntu-rootfs

# Bootstrap Ubuntu (requires debootstrap)
sudo apt install -y debootstrap  # If not installed
sudo debootstrap --arch=amd64 --variant=minbase jammy /tmp/ubuntu-rootfs http://archive.ubuntu.com/ubuntu/

# Configure for Firecracker
sudo chroot /tmp/ubuntu-rootfs /bin/bash << 'EOF'
# Set root password
echo "root:firecracker" | chpasswd

# Configure networking
echo "firecracker-vm" > /etc/hostname
cat > /etc/hosts << HOSTS
127.0.0.1   localhost
127.0.1.1   firecracker-vm
HOSTS

# Install minimal packages
apt update
apt install -y --no-install-recommends openssh-server iproute2 iputils-ping

# Configure console
systemctl enable serial-getty@ttyS0.service

# Exit chroot
exit
EOF

# Unmount
sudo umount /tmp/ubuntu-rootfs
```

## Firecracker Configuration

### Basic VM Configuration

```bash
cd ~/firecracker/configs

# Create basic configuration
cat > vm-config-basic.json << 'EOF'
{
  "boot-source": {
    "kernel_image_path": "../kernels/vmlinux.bin",
    "boot_args": "console=ttyS0 reboot=k panic=1 pci=off"
  },
  "drives": [
    {
      "drive_id": "rootfs",
      "path_on_host": "../rootfs/alpine.ext4",
      "is_root_device": true,
      "is_read_only": false
    }
  ],
  "machine-config": {
    "vcpu_count": 1,
    "mem_size_mib": 128,
    "smt": false
  }
}
EOF

# Create network-enabled configuration
cat > vm-config-network.json << 'EOF'
{
  "boot-source": {
    "kernel_image_path": "../kernels/vmlinux.bin",
    "boot_args": "console=ttyS0 reboot=k panic=1 pci=off"
  },
  "drives": [
    {
      "drive_id": "rootfs",
      "path_on_host": "../rootfs/alpine.ext4",
      "is_root_device": true,
      "is_read_only": false
    }
  ],
  "machine-config": {
    "vcpu_count": 1,
    "mem_size_mib": 128,
    "smt": false
  },
  "network-interfaces": [
    {
      "iface_id": "eth0",
      "guest_mac": "AA:FC:00:00:00:01",
      "host_dev_name": "tap0"
    }
  ]
}
EOF
```

## Network Setup for MicroVMs

### Create Network Bridge

```bash
# Create bridge script
cat > ~/firecracker/setup-network.sh << 'EOF'
#!/bin/bash

# Create bridge
sudo ip link add name fc-bridge type bridge
sudo ip addr add 172.16.0.1/24 dev fc-bridge
sudo ip link set fc-bridge up

# Enable IP forwarding
sudo sysctl -w net.ipv4.ip_forward=1

# Set up NAT
sudo iptables -t nat -A POSTROUTING -o $(ip route | grep default | awk '{print $5}') -j MASQUERADE
sudo iptables -A FORWARD -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT
sudo iptables -A FORWARD -i fc-bridge -o $(ip route | grep default | awk '{print $5}') -j ACCEPT

echo "Bridge fc-bridge created with IP 172.16.0.1/24"
EOF

chmod +x ~/firecracker/setup-network.sh
./setup-network.sh
```

### Create TAP Interface Helper

```bash
cat > ~/firecracker/create-tap.sh << 'EOF'
#!/bin/bash

TAP_DEV=$1
TAP_IP=$2

if [ -z "$TAP_DEV" ] || [ -z "$TAP_IP" ]; then
    echo "Usage: $0 <tap-device> <ip-address>"
    echo "Example: $0 tap0 172.16.0.2"
    exit 1
fi

# Create TAP device
sudo ip tuntap add $TAP_DEV mode tap
sudo ip link set $TAP_DEV master fc-bridge
sudo ip link set $TAP_DEV up

echo "TAP device $TAP_DEV created and added to fc-bridge"
EOF

chmod +x ~/firecracker/create-tap.sh
```

## Launching Your First MicroVM

### Method 1: Using API Socket

```bash
cd ~/firecracker

# Create API socket
rm -f /tmp/firecracker.socket

# Start Firecracker
./bin/firecracker --api-sock /tmp/firecracker.socket &

# Configure VM via API
curl --unix-socket /tmp/firecracker.socket -X PUT 'http://localhost/boot-source' \
    -H 'Content-Type: application/json' \
    -d '{
        "kernel_image_path": "./kernels/vmlinux.bin",
        "boot_args": "console=ttyS0 reboot=k panic=1 pci=off"
    }'

curl --unix-socket /tmp/firecracker.socket -X PUT 'http://localhost/drives/rootfs' \
    -H 'Content-Type: application/json' \
    -d '{
        "drive_id": "rootfs",
        "path_on_host": "./rootfs/alpine.ext4",
        "is_root_device": true,
        "is_read_only": false
    }'

curl --unix-socket /tmp/firecracker.socket -X PUT 'http://localhost/machine-config' \
    -H 'Content-Type: application/json' \
    -d '{
        "vcpu_count": 1,
        "mem_size_mib": 128
    }'

# Start the VM
curl --unix-socket /tmp/firecracker.socket -X PUT 'http://localhost/actions' \
    -H 'Content-Type: application/json' \
    -d '{
        "action_type": "InstanceStart"
    }'
```

### Method 2: Using Configuration File

```bash
cd ~/firecracker

# Start with config file
./bin/firecracker --config-file configs/vm-config-basic.json
```

## Automation Script for Multiple VMs

### Create VM Management Script

```bash
cat > ~/firecracker/vm-manager.sh << 'EOF'
#!/bin/bash

FC_BINARY="./bin/firecracker"
KERNEL="./kernels/vmlinux.bin"
ROOTFS_TEMPLATE="./rootfs/alpine.ext4"
BASE_IP="172.16.0"
BASE_MAC="AA:FC:00:00:00"

start_vm() {
    VM_ID=$1
    
    # Create unique rootfs copy
    cp $ROOTFS_TEMPLATE ./rootfs/vm-${VM_ID}.ext4
    
    # Create TAP interface
    TAP_DEV="tap${VM_ID}"
    sudo ip tuntap add $TAP_DEV mode tap
    sudo ip link set $TAP_DEV master fc-bridge
    sudo ip link set $TAP_DEV up
    
    # Create VM config
    cat > configs/vm-${VM_ID}.json << JSON
{
  "boot-source": {
    "kernel_image_path": "${KERNEL}",
    "boot_args": "console=ttyS0 reboot=k panic=1 pci=off ip=${BASE_IP}.${VM_ID}::${BASE_IP}.1:255.255.255.0::eth0:off"
  },
  "drives": [
    {
      "drive_id": "rootfs",
      "path_on_host": "./rootfs/vm-${VM_ID}.ext4",
      "is_root_device": true,
      "is_read_only": false
    }
  ],
  "machine-config": {
    "vcpu_count": 1,
    "mem_size_mib": 64,
    "smt": false
  },
  "network-interfaces": [
    {
      "iface_id": "eth0",
      "guest_mac": "${BASE_MAC}:$(printf '%02X' $VM_ID)",
      "host_dev_name": "${TAP_DEV}"
    }
  ]
}
JSON
    
    # Start VM in screen session
    screen -dmS vm-${VM_ID} $FC_BINARY --config-file configs/vm-${VM_ID}.json
    
    echo "Started VM ${VM_ID} with IP ${BASE_IP}.${VM_ID}"
}

stop_vm() {
    VM_ID=$1
    
    # Kill screen session
    screen -X -S vm-${VM_ID} quit
    
    # Remove TAP interface
    sudo ip link del tap${VM_ID}
    
    # Clean up files
    rm -f ./rootfs/vm-${VM_ID}.ext4
    rm -f ./configs/vm-${VM_ID}.json
    
    echo "Stopped VM ${VM_ID}"
}

case "$1" in
    start)
        start_vm $2
        ;;
    stop)
        stop_vm $2
        ;;
    start-many)
        COUNT=${2:-10}
        for i in $(seq 2 $((COUNT + 1))); do
            start_vm $i
            sleep 0.5
        done
        ;;
    stop-all)
        for session in $(screen -ls | grep vm- | awk '{print $1}'); do
            screen -X -S $session quit
        done
        sudo ip link show | grep tap | awk '{print $2}' | sed 's/://' | xargs -I {} sudo ip link del {}
        rm -f ./rootfs/vm-*.ext4
        rm -f ./configs/vm-*.json
        echo "All VMs stopped"
        ;;
    status)
        echo "Running VMs:"
        screen -ls | grep vm-
        ;;
    *)
        echo "Usage: $0 {start|stop|start-many|stop-all|status} [vm-id|count]"
        exit 1
        ;;
esac
EOF

chmod +x ~/firecracker/vm-manager.sh
```

## Testing High-Density Deployment

### Launch Multiple VMs

```bash
cd ~/firecracker

# Start 50 microVMs
./vm-manager.sh start-many 50

# Check status
./vm-manager.sh status

# Monitor resource usage
watch -n 1 'free -h; echo; ps aux | grep firecracker | wc -l'
```

### Connect to VMs

```bash
# Connect to a specific VM's console
screen -r vm-5

# Or SSH if you set it up (Ubuntu rootfs)
ssh root@172.16.0.5
```

### Performance Testing Script

```bash
cat > ~/firecracker/perf-test.sh << 'EOF'
#!/bin/bash

echo "=== Firecracker Performance Test ==="
echo "Time: $(date)"
echo

# System info
echo "CPU Cores: $(nproc)"
echo "Total RAM: $(free -h | grep Mem | awk '{print $2}')"
echo

# Test VM startup time
echo "Testing VM startup time..."
START_TIME=$(date +%s.%N)
./bin/firecracker --config-file configs/vm-config-basic.json &
FC_PID=$!
sleep 2
kill $FC_PID 2>/dev/null
END_TIME=$(date +%s.%N)
STARTUP_TIME=$(echo "$END_TIME - $START_TIME - 2" | bc)
echo "VM Startup Time: ${STARTUP_TIME}s"
echo

# Memory per VM
echo "Testing memory usage..."
./vm-manager.sh start 100
sleep 5
VM_COUNT=$(screen -ls | grep vm- | wc -l)
TOTAL_MEM=$(ps aux | grep firecracker | awk '{sum+=$6} END {print sum/1024}')
MEM_PER_VM=$(echo "scale=2; $TOTAL_MEM / $VM_COUNT" | bc)
echo "VMs Running: $VM_COUNT"
echo "Total Memory Used: ${TOTAL_MEM}MB"
echo "Memory per VM: ${MEM_PER_VM}MB"
./vm-manager.sh stop-all

echo
echo "=== Test Complete ==="
EOF

chmod +x ~/firecracker/perf-test.sh
```

## Optimizations for Maximum Density

### 1. Use Shared Rootfs

```bash
# Create read-only base image
cp rootfs/alpine.ext4 rootfs/alpine-base.ext4
chmod 444 rootfs/alpine-base.ext4

# Modify VM config to use read-only rootfs
# Each VM would use an overlay for writes
```

### 2. Reduce Memory Allocation

```bash
# Modify configs to use minimal memory
"mem_size_mib": 32  # or even 16 for very light workloads
```

### 3. Disable SMT

```bash
# Already in configs
"smt": false  # Reduces CPU overhead
```

### 4. Use Kernel Samepage Merging (KSM)

```bash
# Enable KSM
echo 1 | sudo tee /sys/kernel/mm/ksm/run
echo 1000 | sudo tee /sys/kernel/mm/ksm/sleep_millisecs

# Monitor KSM effectiveness
watch cat /sys/kernel/mm/ksm/pages_sharing
```

## Monitoring and Debugging

### System Monitoring

```bash
# Create monitoring script
cat > ~/firecracker/monitor.sh << 'EOF'
#!/bin/bash
while true; do
    clear
    echo "=== Firecracker VM Monitor ==="
    echo "Time: $(date)"
    echo
    echo "Running VMs: $(screen -ls | grep vm- | wc -l)"
    echo
    echo "Memory Usage:"
    free -h
    echo
    echo "CPU Usage:"
    top -bn1 | head -5
    echo
    echo "Firecracker Processes:"
    ps aux | grep firecracker | grep -v grep | awk '{printf "PID: %s CPU: %s%% MEM: %s%%\n", $2, $3, $4}'
    sleep 5
done
EOF

chmod +x ~/firecracker/monitor.sh
```

### Debug VM Issues

```bash
# Check Firecracker logs
journalctl -xe | grep firecracker

# Check specific VM console
screen -r vm-5

# Check network connectivity
ping 172.16.0.5

# Check TAP interfaces
ip link show | grep tap
```

## Cleanup

```bash
# Stop all VMs
~/firecracker/vm-manager.sh stop-all

# Remove network bridge
sudo ip link del fc-bridge

# Remove any leftover TAP interfaces
sudo ip link show | grep tap | awk '{print $2}' | sed 's/://' | xargs -I {} sudo ip link del {}

# Clean up files
rm -f /tmp/firecracker.socket
rm -f ~/firecracker/rootfs/vm-*.ext4
rm -f ~/firecracker/configs/vm-*.json
```

## Next Steps

1. **Automate Bun/Psionic Deployment**: Create rootfs images with Bun pre-installed
2. **Implement Orchestration**: Build a control plane for managing thousands of VMs
3. **Add Monitoring**: Integrate Prometheus/Grafana for detailed metrics
4. **Test Limits**: Push your system to find maximum VM density
5. **Network Optimization**: Implement SDN for better network isolation

This setup provides a foundation for testing Firecracker's capabilities on your home system. You can now experiment with running hundreds of microVMs and understand the resource requirements for hosting Psionic applications at scale.