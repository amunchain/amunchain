# Firewall & hardening (UFW + Fail2Ban)

## UFW
```bash
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw allow 22/tcp
sudo ufw allow 30333/tcp
sudo ufw allow 30334/tcp
sudo ufw allow 30335/tcp
sudo ufw allow 30336/tcp
sudo ufw allow from 127.0.0.1 to any port 9091
sudo ufw allow from 127.0.0.1 to any port 9092
sudo ufw allow from 127.0.0.1 to any port 9093
sudo ufw allow from 127.0.0.1 to any port 9094
sudo ufw limit ssh
sudo ufw enable
```

## Fail2Ban
```bash
sudo apt update && sudo apt install -y fail2ban
sudo tee /etc/fail2ban/jail.local >/dev/null <<'EOF'
[DEFAULT]
bantime = 1h
findtime = 10m
maxretry = 5
backend = systemd

[sshd]
enabled = true
port = ssh
EOF
sudo systemctl enable fail2ban
sudo systemctl restart fail2ban
```
