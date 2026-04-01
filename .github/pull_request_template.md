## 任务卡

关联任务卡：

- [ ] WSA-xx
- [ ] WSB-xx
- [ ] WSC-xx

当前核销状态：`todo | in_progress | review | done | blocked`

## 范围边界

- 本次只处理的工作流与任务范围：
- 明确不处理的内容：
- 相关设计文档：

## 变更内容

- 本次新增或修改了什么：
- 为什么这是最小闭环：
- 是否涉及 schema / artifact / trace / patch / runtime 边界变更：

## 交付物

- [ ] 代码或脚本
- [ ] 测试或 fixture
- [ ] 文档同步
- [ ] 任务状态同步

关键文件：

- 

## 验收标准对照

- [ ] 验收项 1
- [ ] 验收项 2
- [ ] 验收项 3

## 测试与验证

已执行命令：

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo audit
cargo bench --workspace
```

本次实际执行结果：

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo audit`
- [ ] `cargo bench --workspace`（性能相关或里程碑收口时必填）

补充说明：

- 

## 风险与回退

- 当前风险：
- 回退方式：
- 是否存在依赖外部环境的未决项：

## Review 关注点

- [ ] 是否符合任务卡验收标准
- [ ] 是否保持最小闭环，没有额外扩 scope
- [ ] 是否同步更新测试、fixture、文档
- [ ] 是否符合产品边界与架构约束
