/**
 * 春笋文档站 · 内容数据（SSOT）
 *
 * 结构：类目分组 → 功能文档（一个功能一个子菜单）。类目：
 *   getting-started 快速上手 / concepts 核心概念 / guides 功能指南 / reference CLI 参考
 * 渲染：DocsView.vue 消费本数据；内容块类型见 DocBlock。
 * 术语约定：用户侧「自主交付 / 轮次 / 步骤 / 场景 / 用例」（内部 Run/Step 不对外）。
 */

export type DocBlock =
  | { t: "p"; text: string }
  | { t: "h2"; text: string }
  | { t: "h3"; text: string }
  | { t: "ul"; items: string[] }
  | { t: "ol"; items: string[] }
  | { t: "code"; code: string; lang?: string }
  | { t: "note"; kind: "info" | "success" | "warn"; text: string }
  | { t: "table"; head: string[]; rows: string[][] };

export type DocPage = {
  slug: string;
  title: string;
  icon: string;
  desc: string;
  blocks: DocBlock[];
};

export type DocCategory = {
  key: string;
  label: string;
  icon: string;
  desc: string;
  docs: DocPage[];
};

export const docCategories: DocCategory[] = [
  {
    key: "getting-started",
    label: "快速上手",
    icon: "zap",
    desc: "从零接入春笋：装 CLI、建项目、跑起第一次自主交付。",
    docs: [
      {
        slug: "overview",
        title: "快速开始",
        icon: "compass",
        desc: "春笋是什么，四大卖点与第一次自主交付。",
        blocks: [
          {
            t: "p",
            text: "春笋是自部署的 AI 原生项目管理平台。需求、验收、进度留在你的实例上，跨会话、跨用户、跨 Agent 同步续跑。它以「需求」为唯一工作对象，平台保存需求、轮次、步骤、验收场景与工作记忆；本地仓库只负责执行与跑测。在 Agent 中输入 /chunsun <需求ID>，Agent 便会自主推进实施、上报进度、维护验收场景与用例，直到验收全绿交付。",
          },
          {
            t: "note",
            kind: "info",
            text: "在线 Demo：https://chunsun.mengqinghe.com/ —— 访客可直接浏览官网与控制台，注册后可创建项目并跑通第一次 /chunsun。",
          },
          {
            t: "h2",
            text: "整体流程",
          },
          {
            t: "ol",
            items: [
              "在平台创建项目，获取项目密钥（Secret Key）。",
              "本地仓库配置 .env，安装 chunsun CLI。",
              "在仓库内执行 chunsun init，按所选 IDE 接入春笋技能。",
              "在平台录入需求（或登记缺陷）。",
              "在 Agent 中运行 /chunsun <需求ID> 启动自主交付。",
            ],
          },
          {
            t: "note",
            kind: "info",
            text: "全程无需手工拆任务：Agent 自主决策下一步，你只在「需要决策」「验收未通过」「主动打断」三个停点介入。",
          },
          {
            t: "h2",
            text: "核心卖点",
          },
          {
            t: "ul",
            items: [
              "自部署：单二进制平台 + PostgreSQL，数据与密钥留在你的实例。",
              "项目管理：需求、轮次、验收场景、缺陷闭环统一收口，平台为唯一真相源。",
              "跨时空状态同步：进度、决策、工作记忆在平台留存，换会话、换人、换 Agent 可续跑。",
              "多 Agent 支持：Cursor、Claude Code 等 8+ IDE，chunsun init 一键接入。",
            ],
          },
          {
            t: "h2",
            text: "核心思想",
          },
          {
            t: "ul",
            items: [
              "平台是唯一真相源：需求、轮次、步骤、场景、用例、工作记忆都在平台，本地零状态。",
              "信任 Agent：工程化只留「定义」不留「验证」，验收以真实依赖跑通为准。",
              "可续跑：进度与决策留在平台，换会话、换人、换 Agent 都能接着干。",
            ],
          },
        ],
      },
      {
        slug: "install-cli",
        title: "安装 CLI",
        icon: "download",
        desc: "在 macOS / Linux / Windows 上安装 chunsun 命令行工具。",
        blocks: [
          {
            t: "p",
            text: "chunsun CLI 是本地接入春笋的唯一通道：它负责安装技能、读写需求与轮次、维护验收场景，并通过规则提醒层在合适的时机提醒你。",
          },
          {
            t: "h2",
            text: "选择平台并复制安装命令",
          },
          {
            t: "p",
            text: "从已部署实例的文档页获取与你平台匹配的安装命令：",
          },
          {
            t: "table",
            head: ["平台", "命令"],
            rows: [
              ["macOS（Apple Silicon / Intel）", "curl -fsSL <实例>/cli/install.sh | sh"],
              ["Linux（x64 / ARM64）", "curl -fsSL <实例>/cli/install.sh | sh"],
              ["Windows（PowerShell）", "irm <实例>/cli/install.ps1 | iex"],
            ],
          },
          {
            t: "h2",
            text: "验证安装",
          },
          {
            t: "code",
            lang: "bash",
            code: "chunsun --version",
          },
          {
            t: "note",
            kind: "warn",
            text: "若提示找不到命令，请重启终端或检查 PATH 是否包含安装目录。",
          },
          {
            t: "h2",
            text: "升级 CLI",
          },
          {
            t: "p",
            text: "运行 chunsun update 即可检查并更新 CLI 到最新版本：",
          },
          {
            t: "code",
            lang: "bash",
            code: "chunsun update",
          },
          {
            t: "p",
            text: "更新后请重新执行 chunsun init：模板版本变更时会自动刷新仓库内的技能文档，避免仍调用已更名的旧命令。",
          },
        ],
      },
      {
        slug: "project-key",
        title: "创建项目与密钥",
        icon: "folder",
        desc: "在平台创建项目，生成项目密钥并写入本地 .env。",
        blocks: [
          {
            t: "h2",
            text: "创建项目",
          },
          {
            t: "ol",
            items: [
              "登录平台，进入控制台「项目管理」。",
              "点击「新建项目」，填写项目名称与描述。",
              "创建成功后，进入项目详情页。",
            ],
          },
          {
            t: "h2",
            text: "生成项目密钥",
          },
          {
            t: "ol",
            items: [
              "打开项目详情页的「项目设置 → 项目密钥」。",
              "由项目管理员生成密钥（sk_ 前缀）。",
              "将密钥写入本地仓库根目录的 .env 文件。",
            ],
          },
          {
            t: "code",
            lang: "bash",
            code: "CHUNSUN_SECRET_KEY=sk_your-secret-key",
          },
          {
            t: "note",
            kind: "warn",
            text: "密钥等同于凭证，请勿泄露；重新生成会使旧密钥立即失效。",
          },
          {
            t: "h2",
            text: "其他可选环境变量",
          },
          {
            t: "ul",
            items: [
              "CHUNSUN_API_URL：自定义平台 API 地址（默认取已部署实例）。",
              "CHUNSUN_CLI_DOWNLOAD_URL：自定义 CLI 下载地址。",
              "个人私有业务环境变量也可放 .env，同名时优先于平台值。",
            ],
          },
        ],
      },
      {
        slug: "init",
        title: "接入仓库（chunsun init）",
        icon: "link",
        desc: "绑定仓库，按所选 IDE 安装技能、斜线命令与常驻规则。",
        blocks: [
          {
            t: "p",
            text: "chunsun init 会在当前仓库完成三件事：校验密钥并绑定仓库、按所选 IDE 安装技能与斜线命令、在仓库根写入跨 IDE 的常驻桥接段落。",
          },
          {
            t: "code",
            lang: "bash",
            code: "chunsun init",
          },
          {
            t: "h2",
            text: "支持哪些 IDE / Agent",
          },
          {
            t: "ul",
            items: [
              "Cursor、Trae、Qoder、CodeBuddy：安装技能、斜线命令与常驻规则。",
              "Claude Code：额外维护 CLAUDE.md 桥接，规则省略 alwaysApply 即全局加载。",
              "WorkBuddy：仅安装技能。",
              "Agents：技能、斜线命令与常驻规则统一安装到通用 .agents 目录。",
            ],
          },
          {
            t: "h2",
            text: "初始化后仓库里有什么",
          },
          {
            t: "ul",
            items: [
              "对应 IDE 目录下的 skills/chunsun/（春笋技能文档）。",
              "两个斜线命令：/chunsun 与 /chunsun-fix。",
              "常驻规则：chunsun-workflow-gates（各 IDE 独立 frontmatter）。",
              "仓库根 AGENTS.md（或 CLAUDE.md）中的桥接段落，作为跨 IDE 常驻层。",
            ],
          },
          {
            t: "note",
            kind: "info",
            text: "桥接段落按标记幂等维护：重复执行 init 不会重复叠加，也不会改动你手写的其他内容。",
          },
        ],
      },
      {
        slug: "first-run",
        title: "第一次自主交付",
        icon: "play",
        desc: "录入一条需求，用 /chunsun 启动，看到验收全绿。",
        blocks: [
          {
            t: "h2",
            text: "录入需求",
          },
          {
            t: "ol",
            items: [
              "在平台项目内进入「需求管理」。",
              "创建一条需求，描述要做什么、边界是什么。",
              "记下需求 ID（如 AyK8qB3HXfkg）。",
            ],
          },
          {
            t: "h2",
            text: "启动自主交付",
          },
          {
            t: "p",
            text: "在已接入春笋的 Agent 对话中运行：",
          },
          {
            t: "code",
            lang: "bash",
            code: "/chunsun <需求ID>",
          },
          {
            t: "h2",
            text: "会发生什么",
          },
          {
            t: "ol",
            items: [
              "Agent 拉取需求工作记忆与验收场景，开启一个新轮次。",
              "进入循环：决策下一步 → 执行 → 上报步骤 → 更新验收场景与用例。",
              "在三个停点停下：验收全绿（需求完成）/ 需要你决策 / 你主动打断。",
            ],
          },
          {
            t: "note",
            kind: "success",
            text: "验收全绿 = 所有场景均 passing 或 waived，且没有待确认的决策。此时需求状态为「已完成」。",
          },
        ],
      },
    ],
  },
  {
    key: "concepts",
    label: "核心概念",
    icon: "lightbulb",
    desc: "需求、轮次、步骤、验收与工作记忆——自主交付的底层模型。",
    docs: [
      {
        slug: "requirement",
        title: "需求与轮次",
        icon: "check",
        desc: "需求是唯一工作对象，轮次是一次连续工作的时间切片。",
        blocks: [
          {
            t: "p",
            text: "需求（Requirement）是春笋的唯一工作对象。没有功能、应用、模块的层级——你只管理「要做的事」。",
          },
          {
            t: "h2",
            text: "需求状态",
          },
          {
            t: "p",
            text: "需求状态是最新一次连续工作的投影，只有四个取值：",
          },
          {
            t: "table",
            head: ["状态", "含义"],
            rows: [
              ["pending", "从未发起过自主交付"],
              ["running", "最新轮次正在推进（含正常收尾等待下一轮）"],
              ["completed", "最近一轮验收全绿，需求完成"],
              ["abandoned", "已放弃，不再推进"],
            ],
          },
          {
            t: "note",
            kind: "info",
            text: "completed 不是终态：对已完成需求再次 /chunsun，会开启新轮次继续迭代。",
          },
          {
            t: "h2",
            text: "轮次（Run）",
          },
          {
            t: "p",
            text: "每次 /chunsun 都会开启一个新轮次。轮次是「一次连续工作」的时间切片，记录起止、停点与结束原因。",
          },
          {
            t: "ul",
            items: [
              "轮次没有「暂停」：需要停下时，本轮正常收尾（finished），下一次 /chunsun 开启新轮次。",
              "放弃一轮（abandoned）后需求不再推进。",
              "同一需求可叠加多个轮次，历史轮次是各轮结局的快照。",
            ],
          },
        ],
      },
      {
        slug: "steps",
        title: "步骤与上报",
        icon: "compass",
        desc: "轮次内最小工作单元：思考、编码、测试、验收、询问、反思。",
        blocks: [
          {
            t: "p",
            text: "步骤（Step）是轮次内的最小工作单元。Agent 每完成一步都会上报到平台，形成完整的执行轨迹。",
          },
          {
            t: "h2",
            text: "步骤类型",
          },
          {
            t: "table",
            head: ["类型", "说明"],
            rows: [
              ["think", "规划下一步，写下决策思路"],
              ["code", "修改代码，实现功能"],
              ["test", "编写或运行测试"],
              ["verify", "按验收场景执行验证"],
              ["ask_user", "需要你决策，提出问题"],
              ["info", "提供信息或状态说明"],
              ["reflect", "评审 · 反思 · 改进（RRI）"],
            ],
          },
          {
            t: "h2",
            text: "评审 · 反思 · 改进（RRI）",
          },
          {
            t: "p",
            text: "关键环节要做一次评审-反思-改进，以上报 reflect 步骤留痕，例如：编码后进入验收前、场景修复后、准备完成前、收到你的反馈后。",
          },
          {
            t: "note",
            kind: "info",
            text: "RRI 是柔性约束：CLI 只检测「有编码步骤却没有后续反思」这一种可判定情形并提醒，其余靠 Agent 自觉执行。",
          },
        ],
      },
      {
        slug: "scenario",
        title: "验收场景与用例",
        icon: "square-check",
        desc: "场景与用例进平台，验收以真实依赖跑通为准。",
        blocks: [
          {
            t: "p",
            text: "验收场景（Scenario）与用例（Case）是需求的验收标准，唯一真相在平台。它们在自主交付循环中由 Agent 动态涌现并回写，不需要前置规划。",
          },
          {
            t: "h2",
            text: "场景状态",
          },
          {
            t: "table",
            head: ["状态", "含义"],
            rows: [
              ["pending", "待验收"],
              ["passing", "验收通过"],
              ["failing", "验收未通过"],
              ["blocked", "被阻塞"],
              ["waived", "你已自然语言豁免（「这个我认了」）"],
            ],
          },
          {
            t: "h2",
            text: "用例状态",
          },
          {
            t: "p",
            text: "用例挂在场景下，执行状态为 pending / passed / failed / blocked / skipped。完成判定只看场景：全部 passing 或 waived 即可完成需求。",
          },
          {
            t: "h2",
            text: "passing 的标准（宪法条款）",
          },
          {
            t: "ul",
            items: [
              "真实依赖跑通才算 passing：用 stub、假数据或内存替身冒充安全存储、凭据库、系统权限等真实依赖，等同未验收。",
              "端到端用例走真实用户路径，自动用例应有本地路径并跑通。",
              "尚未执行的用例保持 pending，不得伪造 passed。",
            ],
          },
        ],
      },
      {
        slug: "context",
        title: "工作记忆（Context）",
        icon: "database",
        desc: "每个需求一份可跨会话续跑的大脑。",
        blocks: [
          {
            t: "p",
            text: "每个需求都有一份工作记忆（Memory），由平台保存、CLI 增量维护。它是唯一进 Agent 上下文的记忆源，也是断点续跑的关键。",
          },
          {
            t: "h2",
            text: "里面存什么",
          },
          {
            t: "ul",
            items: [
              "requirementSnapshot：已澄清的需求边界（重来保留）。",
              "lastRunSummary：上一轮的决策链与结果。",
              "openDecisions：待你确认的点。",
              "codeLandmarks：关键代码位置与摘要（不存代码本体）。",
              "envRefs：引用的环境变量 key。",
            ],
          },
          {
            t: "note",
            kind: "info",
            text: "场景与用例不存 Context 镜像：启动时从平台实时聚合，保证唯一真相不漂移。",
          },
          {
            t: "h2",
            text: "重来（reset）",
          },
          {
            t: "p",
            text: "说「重做 / 重来」会让 Agent 执行全量重置：清空工作记忆（保留澄清边界）、场景与用例重置为待验收、并开启新轮次。",
          },
        ],
      },
      {
        slug: "defect",
        title: "缺陷闭环",
        icon: "bug",
        desc: "登记缺陷，一条命令派生修复需求，完成即修复。",
        blocks: [
          {
            t: "p",
            text: "春笋支持项目级缺陷管理：登记缺陷（标题、描述、严重级别、状态，可关联需求），并可用 /chunsun-fix 一键派生修复需求。",
          },
          {
            t: "h2",
            text: "缺陷与修复需求 1:1",
          },
          {
            t: "ol",
            items: [
              "在项目「缺陷」页登记缺陷（或从需求创建）。",
              "在 Agent 中运行 /chunsun-fix <缺陷ID>，派生唯一的修复需求并进入自主交付。",
              "修复需求完成后，缺陷自动置为已解决（resolved）。",
            ],
          },
          {
            t: "h2",
            text: "缺陷复发",
          },
          {
            t: "p",
            text: "缺陷复发 = 人工把缺陷拉回未解决状态，再对同一修复需求执行 /chunsun 迭代，不派生新需求。",
          },
          {
            t: "note",
            kind: "warn",
            text: "春笋缺陷模块是轻量登记，不是完整 Bug Tracker：无评论、附件、指派。",
          },
        ],
      },
    ],
  },
  {
    key: "guides",
    label: "功能指南",
    icon: "book-open",
    desc: "平台各功能的用法：项目、需求、缺陷、设置、通知、账户与后台。",
    docs: [
      {
        slug: "projects",
        title: "项目管理",
        icon: "folder",
        desc: "创建项目、成员协作与项目总览。",
        blocks: [
          {
            t: "h2",
            text: "创建项目",
          },
          {
            t: "ol",
            items: [
              "控制台 → 项目管理 → 新建项目。",
              "填写名称与描述，创建后自动成为项目创建者（OWNER）。",
              "在项目详情页查看总览、需求、缺陷与设置。",
            ],
          },
          {
            t: "h2",
            text: "成员与角色",
          },
          {
            t: "p",
            text: "项目成员角色分三档：创建者（OWNER）、管理员（ADMIN）、成员（MEMBER）。角色决定你能对项目做什么，详见「权限与角色」。",
          },
          {
            t: "h2",
            text: "项目总览",
          },
          {
            t: "p",
            text: "总览页聚合项目核心指标：需求数量与状态分布、缺陷、最近活动等，是进入项目后的默认视图。",
          },
        ],
      },
      {
        slug: "requirements",
        title: "需求管理",
        icon: "check",
        desc: "录入需求、查看执行轨迹与验收状态。",
        blocks: [
          {
            t: "h2",
            text: "录入需求",
          },
          {
            t: "ol",
            items: [
              "进入项目 → 需求管理。",
              "创建需求：说明要做什么、边界、验收预期。",
              "保存后获得需求 ID，用于 /chunsun 启动。",
            ],
          },
          {
            t: "h2",
            text: "需求详情",
          },
          {
            t: "p",
            text: "详情页展示需求的最新轮次、步骤轨迹、验收场景与用例状态，以及工作记忆概况——一条需求的完整生命周期都在这里。",
          },
          {
            t: "h2",
            text: "继续迭代",
          },
          {
            t: "p",
            text: "需求完成后想继续改？直接再运行 /chunsun <需求ID>，开启新轮次继续迭代。",
          },
        ],
      },
      {
        slug: "defects",
        title: "缺陷管理",
        icon: "bug",
        desc: "登记缺陷、派生修复需求、跟踪状态联动。",
        blocks: [
          {
            t: "h2",
            text: "登记缺陷",
          },
          {
            t: "ol",
            items: [
              "进入项目 → 缺陷。",
              "新建缺陷：标题、描述、严重级别（可选关联需求）。",
              "保存后即可用 /chunsun-fix <缺陷ID> 进入修复。",
            ],
          },
          {
            t: "h2",
            text: "状态联动",
          },
          {
            t: "table",
            head: ["缺陷状态", "触发"],
            rows: [
              ["open", "登记后"],
              ["processing", "/chunsun-fix 派生修复需求时"],
              ["resolved", "修复需求验收完成时自动置位"],
              ["closed", "人工关闭"],
            ],
          },
        ],
      },
      {
        slug: "settings",
        title: "项目设置",
        icon: "settings",
        desc: "通用设置、成员、密钥、环境变量与项目知识。",
        blocks: [
          {
            t: "h2",
            text: "通用设置",
          },
          {
            t: "p",
            text: "项目名称与描述的修改入口，仅项目管理员可操作。",
          },
          {
            t: "h2",
            text: "成员管理",
          },
          {
            t: "p",
            text: "邀请或移除成员，调整成员角色（OWNER / ADMIN / MEMBER）。",
          },
          {
            t: "h2",
            text: "项目密钥",
          },
          {
            t: "p",
            text: "生成 / 重新生成项目密钥（sk_ 前缀）。重新生成会使旧密钥立即失效。密钥生成与吊销禁止通过 CLI 密钥通道调用，只能在平台操作。",
          },
          {
            t: "h2",
            text: "环境变量",
          },
          {
            t: "p",
            text: "团队共享配置与密钥在平台维护（加密存储），CLI 实时拉取使用，不同步落盘。本地 .env 同名变量优先。环境变量明文取值仅限 CLI 密钥通道。",
          },
          {
            t: "h2",
            text: "项目知识",
          },
          {
            t: "p",
            text: "项目级自定义知识文档，作为全局背景注入 Agent；与需求级工作记忆（Memory）是两层不同机制。",
          },
        ],
      },
      {
        slug: "notifications",
        title: "通知中心",
        icon: "bell",
        desc: "查看平台向你推送的各类通知。",
        blocks: [
          {
            t: "p",
            text: "通知中心汇总平台事件通知：安全告警、项目成员变动、需求与轮次、缺陷与项目变更等。支持已读 / 未读。各分类的站内信与邮件开关可在「账户设置 → 消息通知」中配置。",
          },
        ],
      },
      {
        slug: "account",
        title: "账户设置",
        icon: "user",
        desc: "个人资料、密码与消息通知偏好。",
        blocks: [
          {
            t: "p",
            text: "账户设置包含基本资料、登录密码与消息通知。消息通知按分类控制站内信与邮件；安全类站内信不可关闭。修改后即时生效。",
          },
        ],
      },
      {
        slug: "admin",
        title: "管理后台",
        icon: "shield",
        desc: "平台管理员专用：用户、邀请码与平台设置。",
        blocks: [
          {
            t: "p",
            text: "管理后台仅平台管理员（ADMIN）可访问，入口在控制台侧栏。",
          },
          {
            t: "h2",
            text: "用户管理",
          },
          {
            t: "p",
            text: "查看平台用户列表，调整用户状态或角色（USER / ADMIN）。",
          },
          {
            t: "h2",
            text: "邀请码",
          },
          {
            t: "p",
            text: "生成邀请码，用户凭邀请码注册加入平台（平台开放注册关闭时）。",
          },
          {
            t: "h2",
            text: "平台设置",
          },
          {
            t: "p",
            text: "平台级开关与配置，例如是否开放注册、邮件服务等。",
          },
        ],
      },
      {
        slug: "permissions",
        title: "权限与角色",
        icon: "lock",
        desc: "平台角色 + 项目成员角色双轨 RBAC。",
        blocks: [
          {
            t: "h2",
            text: "双轨认证",
          },
          {
            t: "ul",
            items: [
              "平台角色（USER / ADMIN）：决定平台级能力，如是否可进管理后台。",
              "项目成员角色（OWNER / ADMIN / MEMBER）：决定项目内动作权限。",
            ],
          },
          {
            t: "h2",
            text: "Secret Key 通道",
          },
          {
            t: "p",
            text: "CLI 通过项目密钥调用时，等价于「以项目创建者身份」操作：在项目内拥有 OWNER 级权限。部分写操作（如密钥生成/吊销）禁止密钥通道调用；环境变量明文取值仅密钥通道可读。",
          },
          {
            t: "h2",
            text: "动作判定",
          },
          {
            t: "p",
            text: "项目动作权限由后端策略矩阵统一判定（can_project_action），前端镜像同源生成，保证两端不漂移。",
          },
        ],
      },
    ],
  },
  {
    key: "reference",
    label: "CLI 参考",
    icon: "keyboard",
    desc: "chunsun 命令与斜线命令速查。",
    docs: [
      {
        slug: "commands",
        title: "命令一览",
        icon: "layout-grid",
        desc: "chunsun CLI 全部子命令速查。",
        blocks: [
          {
            t: "p",
            text: "运行 chunsun --help 查看全部命令；以下为核心子命令。",
          },
          {
            t: "code",
            lang: "bash",
            code: `chunsun init                  # 接入：绑定仓库 + 安装 Agent 能力
chunsun requirement …         # list | create | show | update
chunsun defect …              # list | create | show | update | delete | convert-to-requirement
chunsun run …                 # list | start | takeover | status | remind
chunsun step add|list         # 上报 / 查看执行步骤
chunsun scenario …            # list | upsert | status
chunsun case …                # list | upsert | status
chunsun context get|put       # 需求工作记忆
chunsun reset <需求ID>         # 全量重置（重来）
chunsun fix <缺陷ID>           # 派生修复需求并启动自主交付
chunsun env list|get          # 项目环境变量（实时）
chunsun update                # 检查并更新 CLI 到最新版本`,
          },
          {
            t: "h2",
            text: "轮次命令",
          },
          {
            t: "table",
            head: ["命令", "说明"],
            rows: [
              ["run start", "开启新轮次（撞锁时提示接管）"],
              ["run takeover", "接管僵尸轮次后开新轮次"],
              ["run status --status …", "轮次状态迁移（completed / finished / abandoned）"],
              ["run remind", "输出当前未满足的柔性约束"],
            ],
          },
          {
            t: "h2",
            text: "验收命令",
          },
          {
            t: "table",
            head: ["命令", "说明"],
            rows: [
              ["scenario upsert", "创建 / 更新验收场景"],
              ["scenario status", "置场景状态（pending/passing/failing/blocked/waived）"],
              ["case upsert", "创建 / 更新用例"],
              ["case status", "置用例执行状态（passed/failed/blocked/skipped）"],
            ],
          },
          {
            t: "note",
            kind: "warn",
            text: "waived 只能由用户自然语言豁免触发（「这个我认了」），Agent 不得自行豁免。",
          },
        ],
      },
      {
        slug: "slash",
        title: "斜线命令",
        icon: "hash",
        desc: "/chunsun 与 /chunsun-fix：自主交付的两个入口。",
        blocks: [
          {
            t: "table",
            head: ["命令", "用途"],
            rows: [
              ["/chunsun <需求ID>", "启动 / 继续 / 迭代一条需求的自主交付"],
              ["/chunsun-fix <缺陷ID>", "派生唯一修复需求（缺陷 1:1）并进入自主交付"],
            ],
          },
          {
            t: "h2",
            text: "没有 /暂停 /重来 命令",
          },
          {
            t: "ul",
            items: [
              "暂停 = 在会话中直接打断，当前步骤收尾后停下。",
              "重来 = 用自然语言说明（「把登录那块重做」），Agent 判断意图后执行全量重置。",
            ],
          },
          {
            t: "h2",
            text: "自主交付循环",
          },
          {
            t: "ol",
            items: [
              "拉取需求工作记忆与验收场景聚合，开启新轮次。",
              "循环：决策下一步 → 执行 → 上报步骤 → 更新场景与用例。",
              "停点：验收全绿 / 需要你决策（ask_user）/ 你打断。",
            ],
          },
          {
            t: "note",
            kind: "info",
            text: "completed 是平台硬条件：场景须全部 passing 或 waived 且无待确认决策，否则平台拒绝完成。",
          },
        ],
      },
      {
        slug: "env",
        title: "环境变量",
        icon: "key",
        desc: "平台环境变量：团队共享、加密存储、CLI 实时拉取。",
        blocks: [
          {
            t: "h2",
            text: "工作机制",
          },
          {
            t: "ul",
            items: [
              "平台保存团队共享的配置与密钥，加密存储。",
              "CLI 通过项目密钥实时拉取，不同步落盘。",
              "本地 .env 同名变量优先于平台值。",
            ],
          },
          {
            t: "h2",
            text: "常用命令",
          },
          {
            t: "code",
            lang: "bash",
            code: `chunsun env list    # 列出环境变量键（不含明文）
chunsun env get <KEY>  # 获取单个变量值`,
          },
          {
            t: "note",
            kind: "warn",
            text: "环境变量明文取值仅限 CLI 密钥通道（本地 .env 优先）；平台 Web 界面不展示明文。",
          },
        ],
      },
    ],
  },
];

/** 全部文档扁平化（用于查找、默认页等） */
export const allDocs: { category: DocCategory; doc: DocPage }[] = docCategories.flatMap(
  (category) => category.docs.map((doc) => ({ category, doc })),
);

export function findDoc(slug: string) {
  return allDocs.find((entry) => entry.doc.slug === slug);
}

/** 默认文档：快速上手的 overview */
export const defaultDocSlug = "overview";
