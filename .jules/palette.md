## 2024-05-23 - Visibility of Subprocess Logs
**Learning:** Python's `subprocess.Popen` with `stderr=subprocess.PIPE` (without reading it) hides critical build progress and error logs from the user, leading to a "hanging" experience during long operations like `cargo run`. It can also cause deadlocks if the buffer fills up.
**Action:** When wrapping CLI tools that output useful progress/logs to stderr (like cargo, git, npm), set `stderr=None` (or inherit) so the user sees real-time feedback. This improves perceived performance and trust.
