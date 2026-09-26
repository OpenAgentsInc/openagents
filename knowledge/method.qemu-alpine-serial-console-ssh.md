---
id: method.qemu-alpine-serial-console-ssh
version: 1
kind: method
title: Boot Alpine under headless QEMU and configure SSH through a serial console
summary: >-
  A reliable sequence for bringing up a headless Alpine VM: attach a
  host-forwarded SSH port, use a Unix serial socket for console input/output,
  install and enable SSH, then verify a real host login. Applies when
  provisioning a minimal or live Alpine guest without graphical access.
tags: [qemu, alpine, ssh, serial-console, virtualization]
applies_when: >-
  A coding agent must configure an Alpine Linux guest running under headless
  QEMU and provide host-accessible SSH.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - qemu-alpine-ssh
  cites:
    - QEMU Project, QEMU System Emulation User's Guide, sections “Network emulation” and “Invocation”
    - Alpine Linux Wiki, OpenRC, “Managing services”
    - Alpine Linux Wiki, Setting up a SSH server
    - OpenBSD Project, sshd_config(5), PermitRootLogin and PasswordAuthentication
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Start with a writable guest disk and an Alpine installation/live image, and explicitly choose the boot device. In headless mode, expose serial I/O through a Unix-domain socket and forward a host loopback TCP port to guest TCP/22, for example:

```sh
qemu-system-x86_64 -m 1024 -cdrom alpine.iso -boot d \
  -drive file=guest.qcow2,format=qcow2,if=virtio \
  -nic user,model=virtio-net-pci,hostfwd=tcp:127.0.0.1:2222-:22 \
  -display none -serial unix:/tmp/guest-serial.sock,server,nowait \
  -monitor unix:/tmp/guest-monitor.sock,server,nowait \
  -pidfile /tmp/guest.pid -daemonize
```

Inspect the existing VM/process and test the host port before launch to avoid conflicts. QEMU's human monitor is a command interface, not a shell: `sendkey` can drive the emulated keyboard, while the serial socket is the appropriate place to read a Linux text console. If the boot menu or kernel does not send its console to serial, use the monitor to select a kernel command line containing `console=ttyS0,115200`, or log in through the emulated display/keyboard and arrange a serial getty.

In the guest, bring up its NIC and obtain a DHCP lease before installing packages. Configure the guest's network manager/interfaces for boot persistence; a successful DHCP command in a live session alone does not guarantee networking after restart. Install OpenSSH with Alpine's package manager, set the requested account credentials, set the SSH daemon policy deliberately, and enable both networking and sshd at boot using the guest's OpenRC services. Password authentication and root SSH access are security-sensitive: enable them only when explicitly required, preferably bind QEMU forwarding to loopback, and use a strong task-provided credential rather than reusing a fixed demonstration password.

Check the effective daemon configuration and service state, not merely the text of a config file. Finally, authenticate from the host with a noninteractive test harness (such as Expect) that handles first-contact host-key confirmation, waits for the password prompt, and verifies an interactive shell or a command's output. A TCP connection or SSH banner proves only that a server is listening, not that credentials work.

Sources: QEMU Project, *QEMU System Emulation User's Guide*, sections “Network emulation” and “Invocation”; Alpine Linux Wiki, *OpenRC*, “Managing services”; Alpine Linux Wiki, *Setting up a SSH server*; OpenSSH, *sshd_config(5)*, options `PermitRootLogin` and `PasswordAuthentication`.

## How to check

From the host, verify an actual authenticated session and root identity (substitute the requested credentials):

```sh
expect <<'EOF'
set timeout 30
spawn ssh -o PubkeyAuthentication=no -o PreferredAuthentications=password -p 2222 root@127.0.0.1
expect {"yes/no" {send "yes\r"; exp_continue} "password:" {send "PASSWORD\r"} timeout {exit 1} eof {exit 1}}
expect {"#" {} timeout {exit 1} eof {exit 1}}
send "id\r"
expect {"uid=0(root)" {} timeout {exit 1}}
send "exit\r"
expect eof
EOF
```

Also inspect the guest's effective SSH policy and OpenRC service status (for example, `sshd -T` and `rc-service sshd status`). Confirm the QEMU process remains alive if the VM is expected to persist in the background.
