# Changelog


## 0.1.2 (2025/09/14)
### Changed
- Moved `ToFlatbuffers` trait from `config.rs` to `mod.rs` for centralized API and easier imports.
- Use trait instead of function as converter

### Fix
- Creating a `TaskMonitor` with an empty task map now returns a `ConfigParse` error, ensuring proper validation and error reporting.

## 0.1.1 (2025/09/14)
- Support stdin and task termination
- Add documents
  
## 0.1.0 (2025/09/09)

- Initial project setup, split project from tcrm
- Add tests
