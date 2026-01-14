# Public Testnet (amunchain-testnet-1)

Bootstrap:
- IP: 72.60.33.99
- P2P port: 30333
- PeerId: (fill in once extracted from node1)

## Extract bootstrap PeerId (on bootstrap server)
```bash
/srv/amunchain/bin/amunchain --print-peer-id /srv/amunchain/node1
```

## Make a distributable join kit
Copy the following to a public location (or share as an archive):
- public-testnet/amunchain
- public-testnet/genesis.json
- public-testnet/validators.json
- public-testnet/join_testnet.sh
- public-testnet/README.md
