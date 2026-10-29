# Security

## Reporting a problem

Please report a security problem privately: use **Security › Report a vulnerability** on this repository (GitHub private advisory), or open an issue that only says you have something to report and we will contact you. Do not post details in a public issue.

## What Datum Schema does and does not do

- Projects are `.dschema` files (JSON) on your own computer.
- The only network call is the update check and download from this repository's GitHub Releases. Every download is checked against its SHA-256 checksum (`SHA256SUMS.txt`) before it installs; a release without one is never installed.
- The window can only open links to the Datum repositories and site (`open_url` refuses anything else).
- Project files are treated as untrusted: the reader is tested with damaged and random files and must never crash the app; a damaged file opens as far as possible and lists what it left out. Text in tags and notes is always shown as text, never run.
- Every save is atomic (a temporary file, then a rename), and every file read has a size limit.
- The installer is not code-signed yet (planned for 1.0), so Windows shows a SmartScreen warning on the first install.
