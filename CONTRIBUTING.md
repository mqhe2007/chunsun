# 贡献指南

感谢你对春笋的关注！本项目欢迎来自社区的各类贡献——提交 Bug、提出功能建议、改进文档，或直接提交代码。

春笋采用标准的 GitHub **Fork & Pull Request** 流程。

## 行为准则

参与本项目即表示同意遵守 [行为准则](./CODE_OF_CONDUCT.md)。

## 开始之前

- 按 README「本地开发」章节在本地跑通环境。
- **重大功能或架构变更，请先开 Issue 讨论**，确认方向后再动手，避免重复劳动或方向分歧。
- 确保你使用的是项目指定的工具链版本：
  - Node.js 22+
  - pnpm 11.21.0
  - Rust stable（后端与 CLI）
  - PostgreSQL（本地开发所需）

## 贡献流程

### 1. Fork 并创建分支

Fork 本仓库，克隆到本地，基于最新 `main` 创建工作分支：

```bash
git remote add upstream https://github.com/<owner>/chunsun.git
git fetch upstream
git checkout -b feat/your-feature upstream/main
```

分支命名建议：`feat/`、`fix/`、`docs/`、`refactor/`、`test/`、`chore/` 前缀 + 简短描述。

### 2. 开发与提交

保持提交粒度合理，每个提交聚焦一个逻辑变更。遵循[约定式提交](https://www.conventionalcommits.org/)规范：

| 类型 | 说明 |
|------|------|
| `feat:` | 新功能 |
| `fix:` | Bug 修复 |
| `docs:` | 文档变更 |
| `refactor:` | 重构（不改变功能） |
| `test:` | 测试相关 |
| `chore:` | 构建、工具、依赖等杂项 |
| `perf:` | 性能优化 |
| `style:` | 代码格式（不影响逻辑） |

示例：

```bash
git commit -m "feat(cli): add chunsun env list command"
git commit -m "fix(backend): handle null project_id in requirement query"
```

### 3. 本地跑通质量门禁

提交前请在本地跑通与 CI 一致的检查：

```bash
# 前端（website + console）
pnpm --filter website typecheck
pnpm --filter website test
pnpm --filter console typecheck
pnpm --filter console test

# 后端
cargo test --manifest-path packages/backend/Cargo.toml

# CLI
cargo test --manifest-path packages/cli/Cargo.toml
```

如变更涉及前端构建，也可运行：

```bash
pnpm fe:build
```

### 4. 发起 Pull Request

- **标题**：清晰描述变更，建议沿用约定式提交格式（如 `feat(scope): description`）。
- **正文**：
  - 关联相关 Issue（如 `Closes #123`、`Refs #456`）。
  - 说明变更动机与背景。
  - 概述实现方式。
  - 列出测试覆盖情况与验证方式。
  - 说明影响范围、是否包含破坏性变更。
- **保持 PR 聚焦单一主题**，避免无关改动混入；如需多个独立变更，请拆成多个 PR。
- 若 PR 尚未完成，可标记为 Draft。

### 5. 代码评审

- 维护者会进行 Review，可能要求修改、补充测试或澄清设计。
- 请及时响应评论，将修改推送至同一分支（无需新建 PR）。
- Review 过程中如方向有重大调整，建议回到 Issue 讨论后再继续。

### 6. 合并

PR 通过评审且 CI 全绿后，由维护者合并入 `main`。合并方式由维护者根据提交粒度决定（Squash / Rebase / Merge）。

## 报告 Bug 与提出建议

### Bug 报告

提交 Bug 时请尽量提供：

- 复现步骤
- 预期行为
- 实际行为
- 环境信息：OS、春笋版本、CLI 版本、相关日志或截图

### 功能建议

提交功能建议时请说明：

- 动机与使用场景
- 期望行为
- 可选的实现思路（如有）

### 安全漏洞

**请勿在公开 Issue 中披露安全漏洞。** 请通过仓库维护者私下联系，提供漏洞详情、复现步骤与影响范围。

## 文档与非代码贡献

文档改进、示例补充、翻译、错别字修正同样欢迎，直接提 PR 即可。文档变更同样需要经过 Review。

## 贡献者许可

提交贡献即表示你同意其内容以本项目的 [MIT License](./LICENSE) 开源，并保证你有权授予该许可。
