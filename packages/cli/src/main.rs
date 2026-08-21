use std::process::ExitCode;

use clap::{ArgAction, CommandFactory, Parser, Subcommand};
use chunsun::bootstrap::{bootstrap_secret_key, should_skip_bootstrap};
use chunsun::commands;
use chunsun::load_runtime_env;
use chunsun::version;

#[derive(Parser)]
#[command(
    name = "chunsun",
    about = "春笋 CLI 工具",
    version = version(),
    disable_version_flag = true,
    propagate_version = true
)]
struct Cli {
    /// 打印版本号
    #[arg(short = 'v', long = "version", action = ArgAction::Version, global = true)]
    _version: (),

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 安装/刷新技能文件，并在平台注册/绑定仓库
    Init(commands::init::InitArgs),
    /// 管理当前项目下的 repository 注册
    Repo(commands::repo::RepoArgs),
    /// 项目环境变量
    Env(commands::env::EnvArgs),
    /// 管理项目需求
    Requirement(commands::requirement::RequirementArgs),
    /// 管理项目缺陷
    Defect(commands::defect::DefectArgs),
    /// 自主交付轮次管理（/chunsun 协议）
    Run(commands::harness::RunArgs),
    /// 自主交付步骤上报
    Step(commands::harness::StepArgs),
    /// 验收场景管理
    Scenario(commands::harness::ScenarioArgs),
    /// 测试用例管理
    Case(commands::harness::CaseArgs),
    /// 全量重置（重来：清工作记忆 + 场景/用例重置 pending + 开新 Run）
    Reset(commands::harness::ResetArgs),
    /// 缺陷派生修复需求并启动自主交付（/chunsun-fix）
    Fix(commands::harness::FixArgs),
    /// 检查并更新 chunsun CLI 到最新版本
    Update(commands::update::UpdateArgs),
    /// 项目开发上下文摘要；子命令 get|put 管理需求工作记忆
    Context(commands::context::ContextArgs),
    /// （内部）刷新当前仓库的春笋技能模板
    #[command(name = "_refresh-templates", hide = true)]
    RefreshTemplates,
}

fn main() -> ExitCode {
    commands::update::cleanup_stale_update();

    if let Err(err) = load_runtime_env(None) {
        eprintln!("[chunsun] {err}");
        return ExitCode::from(1);
    }

    let argv: Vec<String> = std::env::args().skip(1).collect();
    if !should_skip_bootstrap(&argv) {
        if let Err(err) = bootstrap_secret_key() {
            eprintln!("[chunsun] {err}");
            return ExitCode::from(1);
        }
    }

    let cli = Cli::parse();
    let result = match cli.command {
        None => {
            // clap prints help when no subcommand if we re-parse with help; mimic commander
            let mut cmd = Cli::command();
            let _ = cmd.print_help();
            println!();
            Ok(())
        }
        Some(Commands::Init(args)) => commands::init::run(args),
        Some(Commands::Repo(args)) => commands::repo::run(args),
        Some(Commands::Env(args)) => commands::env::run(args),
        Some(Commands::Requirement(args)) => commands::requirement::run(args),
        Some(Commands::Defect(args)) => commands::defect::run(args),
        Some(Commands::Run(args)) => commands::harness::run_run(args),
        Some(Commands::Step(args)) => commands::harness::run_step(args),
        Some(Commands::Scenario(args)) => commands::harness::run_scenario(args),
        Some(Commands::Case(args)) => commands::harness::run_case(args),
        Some(Commands::Reset(args)) => commands::harness::run_reset(args),
        Some(Commands::Fix(args)) => commands::harness::run_fix(args),
        Some(Commands::Update(args)) => commands::update::run(args),
        Some(Commands::Context(args)) => commands::context::run(args),
        Some(Commands::RefreshTemplates) => commands::update::run_refresh_templates(),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let code = err.exit_code();
            if !err.is_silent() {
                eprintln!("[chunsun] {err}");
            }
            ExitCode::from(code)
        }
    }
}
