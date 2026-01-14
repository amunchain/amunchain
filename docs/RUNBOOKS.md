# Enterprise Runbooks

## 1) Consensus Stall (no finality / stuck height)
**Symptoms**
- Finalized height not increasing for > N seconds/minutes
- Increased view-change events
- Peers disconnect/reconnect spikes

**Immediate Actions**
1. Confirm is it one node or cluster-wide:
   - Check metrics: height, view, peer count, invalid msg rate
2. Check resource saturation:
   - CPU/mem throttling, OOMKilled, disk full
3. Identify partition:
   - Compare peer lists and latency between nodes

**Containment**
- If one node misbehaves: cordon/isolated restart
- If widespread: temporarily reduce ingress (rate limit) and keep only trusted peers

**Recovery**
- Restart the least healthy nodes first (one at a time)
- Ensure time sync is correct
- Verify storage integrity / snapshots

**Postmortem**
- Capture logs, metrics window, configuration hash
- Add regression tests for the triggering pattern

---

## 2) Suspected Key Compromise
**Symptoms**
- Unexpected signatures, equivocation alerts, unknown validator actions

**Immediate Actions**
1. Rotate keys immediately (KMS/Vault) and revoke old credentials.
2. Quarantine affected validator(s): remove from active set if supported.
3. Audit recent deployments and access logs.

**Recovery**
- Re-issue certs, re-enroll nodes
- Review least-privilege and secrets mount policies

---

## 3) DoS / Flood Attack
**Symptoms**
- High connection churn, high invalid message rate, CPU pegged

**Immediate Actions**
1. Confirm rate limiting is active (conn limits, token buckets).
2. Tighten NetworkPolicy to allow only known peer CIDRs temporarily.
3. Increase drop aggressiveness in peer scoring (quarantine/bans).

**Recovery**
- Scale horizontally if applicable
- Add additional ingress protections (L4 LB, SYN cookies, DDoS provider)

---

## 4) State Corruption / DB Errors
**Symptoms**
- Crash loops, snapshot load failure, checksum mismatch

**Immediate Actions**
1. Stop affected node(s) to prevent further writes.
2. Restore from last known good snapshot.
3. Validate chain/state hashes against other validators.

**Postmortem**
- Identify root cause (unclean shutdown, disk issues, bug)
- Add checksums, fsync discipline, and corruption tests
