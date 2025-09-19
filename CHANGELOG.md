# Changelog

## 0.1.4 (2025/09/19)
### Fix
- update `tcrm-task` dependency to version 0.3.6, fix grandchild processes hanging problem by using Job/Process group
## 0.1.3 (2025/09/18)
### Fix
- incorrect `completed_tasks` report in event `Completed` [[commit](https://github.com/xpcn2015/tcrm-monitor/commit/3f62f38eb211971cbc02b38f733c5d91e1e73a68)]

## 0.1.2 (2025/09/14)
### Changed
- Moved `ToFlatbuffers` trait from `config.rs` to `mod.rs` for centralized API and easier imports.
- Use trait instead of function as converter
- implement FlatBuffers conversion for TaskEvent types in TaskMonitorEvent [[commit](https://github.com/xpcn2015/tcrm-monitor/commit/8765b6dc6c16f51950c2a251a70c28af52123460)]

### Fix
- Creating a `TaskMonitor` with an empty task map now returns a `ConfigParse` error, ensuring proper validation and error reporting.

## 0.1.1 (2025/09/14)
- Support stdin and task termination
- Add documents
  
## 0.1.0 (2025/09/09)

- Initial project setup, split project from tcrm
- Add tests
