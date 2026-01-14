# Patch7 Notes

Patch7 focuses on making fuzzing **operational**:

- Adds a small checked-in corpus for each fuzz target
- Adds repeatable crash triage + minimization (`scripts/triage_fuzz.sh`)
- Updates CI to always upload corpus + artifacts
- Adds a scheduled nightly fuzz workflow for longer runs

