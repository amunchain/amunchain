# 4 Nodes on one VPS (systemd)

Create a template service:

`/etc/systemd/system/amunchain@.service`

```ini
[Unit]
Description=Amunchain Node %i
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=amunchain
Group=amunchain
ExecStart=/srv/amunchain/bin/amunchain /etc/amunchain/node%i.toml
Restart=always
RestartSec=3
StartLimitIntervalSec=30
StartLimitBurst=5

NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
LockPersonality=true
RestrictRealtime=true
MemoryDenyWriteExecute=true

LimitNOFILE=1048576
LimitNPROC=4096
MemoryMax=2G
CPUQuota=80%

StandardOutput=journal
StandardError=journal
SyslogIdentifier=amunchain-node%i

[Install]
WantedBy=multi-user.target
```

Enable/start nodes:
```bash
for i in 1 2 3 4; do
  sudo systemctl enable amunchain@$i
  sudo systemctl start amunchain@$i
done
```
