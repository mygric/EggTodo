# EggDone v1.2.0 更新日志

**发布日期：** 2026-08-28

## 亮点

桌面悬浮球全新上线！不用再找托盘图标，桌面上一颗蛋仔随时待命，点击即开即关，未完成任务数实时可见。

---

## 新增功能

### 桌面悬浮球

- **可自由拖拽**：按住悬浮球拖动到屏幕任意位置，位置自动记忆，下次启动恢复
- **点击即开即关**：单击悬浮球显示主面板，再次单击隐藏，响应稳定无延迟
- **未完成计数角标**：悬浮球右上角显示今日未完成任务数量，角标位置精准贴合蛋仔手中的勾选牌
- **计数实时同步**：在主面板内完成任务时，悬浮球角标数字自动更新，无需手动刷新
- **悬停动效**：鼠标移到悬浮球上有缩放反馈，交互更灵动
- **默认位置**：首次启动出现在屏幕右上角，距边缘约 2 厘米
- **纯透明背景**：悬浮球窗口完全透明，无边框无阴影，只有蛋仔本体

### 设置 - 悬浮球开关

- 在「设置 → 通用 → 启动默认视图」下方新增「悬浮球」开关
- 开关样式与软件内其他开关保持统一
- 关闭后悬浮球立即隐藏，开启后立即显示，状态持久化保存

### 主窗口圆角

- 主面板从直角改为 18px 圆角，视觉更柔和
- 搭配细边框和顶部内阴影高光，质感升级
- 窗口背景透明，圆角外区域完全透明无残影

---

## 功能改进

### 重复任务

- 取消完成重复任务时，自动删除该任务所有后续未完成的重复实例，避免残留无效任务
- 修复 `normalize_repeat_rule` 规则校验，允许所有合法重复规则通过

### 数据同步与导入导出

- 修复 `sync_settings` 表未自动创建的问题，首次启动同步功能不再报错
- 完整修复 JSON 导入导出功能，设置、分组、任务数据均可完整备份和恢复

---

## 下载

| 平台 | 下载 |
|------|------|
| Windows | [EggDone_1.2.0_x64-setup.exe](https://github.com/mygric/EggTodo/releases/download/v1.2.0/EggDone_1.2.0_x64-setup.exe) |
| Windows (便携版) | [EggDone_1.2.0_x64-portable.exe](https://github.com/mygric/EggTodo/releases/download/v1.2.0/EggDone_1.2.0_x64-portable.exe) |
| macOS | [EggDone_1.2.0_aarch64.dmg](https://github.com/mygric/EggTodo/releases/download/v1.2.0/EggDone_1.2.0_aarch64.dmg) |
| Linux | [EggDone_1.2.0_amd64.deb](https://github.com/mygric/EggTodo/releases/download/v1.2.0/EggDone_1.2.0_amd64.deb) |

> 鸿蒙移动版请在[华为应用市场](https://appgallery.huawei.com/app/detail?id=com.eggdone.todo)搜索「蛋定 Todo」下载。
