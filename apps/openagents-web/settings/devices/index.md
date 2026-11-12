# Your devices

[openagents.com/settings/devices](https://openagents.com/settings/devices) lists the phones and
computers that hold a login to your account from the OpenAgents app. Log in first.

## What a device key is

When the app logs in, it makes a key that can't leave the device: in the Secure Enclave on an
iPhone or a Mac, and in the Android Keystore (StrongBox when the phone has it) on Android. The
login is tied to that key. Each time the app renews its login, it signs the request with the key,
so a login copied off the device, from a backup or a log, doesn't work anywhere else.

A device with no such hardware keeps its key in a file only that device's user can read. The list
says where each device keeps its key. That is what the app tells us; we don't ask the hardware to
prove it.

A device key isn't a way to log in, and it doesn't replace your email, key or passkey.

## The list

Each device shows its name, what it is, where its key is kept, when it was last used and how many
logins it holds. The page shows the 8 devices used most recently and says when you have more.

## Remove a device

Press REMOVE, then PRESS AGAIN TO REMOVE. The device's key stops working and every login on that
device ends at once. To use that device again, log in on it again.

If you don't recognize a device, remove it, then freeze your account on
[/settings/data](/settings/data/index.md) if you think someone else got in.

To see how you log in, go to [/settings/security](/settings/security/index.md).
