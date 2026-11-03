mod app;

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use costline_core::{config, monitor};

#[derive(Parser)]
#[command(
    name = "costline",
    version,
    about = "股票 / 基金 / 黄金 回本盯盘与通知工具"
)]
struct Cli {
    /// 配置文件路径
    #[arg(short, long, global = true, env = "COSTLINE_CONFIG")]
    config: Option<PathBuf>,

    /// 状态文件路径
    #[arg(long, global = true, env = "COSTLINE_STATE")]
    state: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 生成示例配置文件
    Init {
        /// 覆盖已存在的配置文件
        #[arg(long)]
        force: bool,
    },
    /// 检查一次行情
    Check {
        /// 只检查，不发送通知
        #[arg(long)]
        no_notify: bool,
        /// 以 JSON 输出结果（通知状态仍输出到 stderr）
        #[arg(long)]
        json: bool,
        /// 忽略冷却与状态，强制发送通知
        #[arg(long)]
        force_notify: bool,
    },
    /// 持续盯盘
    Watch {
        /// 只检查，不发送通知
        #[arg(long)]
        no_notify: bool,
    },
    /// 发送一条测试通知
    TestNotify,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("错误：{error:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let config_path = cli.config.unwrap_or_else(config::default_config_path);
    let state_path = cli.state.unwrap_or_else(config::default_state_path);

    match cli.command {
        Command::Init { force } => {
            if config_path.exists() && !force {
                bail!(
                    "配置文件已存在：{}（如需覆盖请加 --force）",
                    config_path.display()
                );
            }
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("创建目录失败：{}", parent.display()))?;
            }
            std::fs::write(&config_path, config::SAMPLE_CONFIG)
                .with_context(|| format!("写入配置文件失败：{}", config_path.display()))?;
            println!("已生成配置文件：{}", config_path.display());
            println!("请编辑其中的资产与通知渠道，然后运行：costline check");
            Ok(())
        }
        Command::Check {
            no_notify,
            json,
            force_notify,
        } => {
            let config = config::load(&config_path)?;
            let client = monitor::build_client(&config)?;
            app::run_once(
                &config,
                &client,
                &state_path,
                !no_notify,
                force_notify,
                json,
            )
            .await
        }
        Command::Watch { no_notify } => app::watch(&config_path, &state_path, !no_notify).await,
        Command::TestNotify => {
            let config = config::load(&config_path)?;
            if config.notify.channels.is_empty() {
                bail!(
                    "没有配置通知渠道，请编辑 {} 中的 [[notify.channels]]",
                    config_path.display()
                );
            }
            let client = monitor::build_client(&config)?;
            let notice = monitor::test_notice();
            let results =
                costline_core::notify::send_all(&client, &config.notify.channels, &notice).await;
            let mut success = 0;
            for result in results {
                match result.result {
                    Ok(()) => {
                        success += 1;
                        println!("✅ {} 发送成功", result.label);
                    }
                    Err(error) => {
                        eprintln!("❌ {} 发送失败：{error:#}", result.label);
                    }
                }
            }
            if success == 0 {
                bail!("所有通知渠道均发送失败");
            }
            println!("测试通知完成：{success} 个渠道成功");
            Ok(())
        }
    }
}
