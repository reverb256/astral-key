# Mosiac Identity on Termux

Run Mosiac on any Android device via Termux — no root, no Docker, no compilation.

## Quick Start

```bash
# 1. Install Termux from F-Droid (NOT Google Play — it's outdated)
#    https://f-droid.org/packages/com.termux/

# 2. Update packages and install Node.js
pkg update && pkg upgrade -y
pkg install nodejs git -y

# 3. Download Mosiac
git clone https://github.com/reverb256/Mosaic
cd Mosaic

# 4. Start the identity service
npm start
```

## What Happens

- `better-sqlite3` will fail to compile (no C++ toolchain in Termux by default)
- The sql.js WASM fallback kicks in automatically
- Works with zero native dependencies

## Make It Persistent

Run in a `tmux` session so it survives backgrounding:

```bash
pkg install tmux -y
tmux new -s mosiac
npm start
# Ctrl+B, D to detach
# tmux attach -t mosiac to reattach
```

Or use Termux:Boot to start on device boot:

```bash
mkdir -p ~/.termux/boot
cat > ~/.termux/boot/mosiac.sh << 'EOF'
#!/data/data/com.termux/files/usr/bin/sh
cd ~/Mosiac && npm start
EOF
chmod +x ~/.termux/boot/mosiac.sh
```

## Access From Other Devices

Your phone's IP + port 3002. Find it with:

```bash
ifconfig
# or
ip addr show
```

Open `http://<phone-ip>:3002/identity.html` on any device on the same network.

## Running Alongside Other Services

Mosiac runs on port 3002 by default. Change with:

```bash
MOSIAC_PORT=8080 npm start
```

## Why Termux?

Termux turns any Android device into a Linux server. Combining it with Mosiac means:
- Your identity lives on your phone, not in a cloud
- QR codes for key exchange work natively with the phone camera
- Passkeys use the device biometrics
- The device is always with you
