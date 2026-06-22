# Arcana Table

一个使用 Rust + Bevy 构建的卡牌游戏原型。目前包含可交互、可自适应窗口大小的游戏主界面。

## 运行

```powershell
cargo run
```

首次构建 Bevy 依赖会花一些时间。开发构建已在 `Cargo.toml` 中为依赖开启优化，以改善运行时渲染性能。

## 当前内容

- 原生 Bevy UI 主菜单
- 新冒险、继续、牌组、设置和退出入口
- 鼠标悬停与点击反馈
- 程序化卡牌展示，无外部美术资源依赖
- 窄窗口响应式布局

## Computer opponent

New Adventure starts a two-player game with the human as Player 1 (`YOU`) and a computer as Player 2 (`CPU`). The computer uses a hidden-information MCTS search for up to one second per action, discard, or noble choice. Blind-reserved cards remain hidden from the opponent.

AI correctness tests run with `cargo test`. The longer deterministic strength check is available with:

```powershell
cargo test mcts_beats_random_at_least_sixty_five_percent -- --ignored --nocapture
```

