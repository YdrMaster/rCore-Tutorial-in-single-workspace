# 任务管理

[![Latest version](https://img.shields.io/crates/v/rcore-task-manage.svg)](https://crates.io/crates/rcore-task-manage)
[![Documentation](https://docs.rs/rcore-task-manage/badge.svg)](https://docs.rs/rcore-task-manage)
![license](https://img.shields.io/github/license/YdrMaster/rCore-Tutorial-in-single-workspace)

#### 事先申明：对于 `feature` 的使用不太熟悉，所以代码不是很优雅

#### 任务 id 类型，自增不回收，任务对象之间的关系通过 id 类型来实现
* `ProcId`
* `ThreadId`
* `CoroId`
#### 任务对象管理 `manage trait`，对标数据库增删改查操作
* `insert`
* `delete`
* `get_mut`
#### 任务调度 `schedule trait`，队列中保存需要调度的任务 `Id`
* `add`：任务进入调度队列
* `fetch`：从调度队列中取出一个任务
#### 封装任务之间的关系，使得 `PCB`、`TCB` 内部更加简洁
* `ProcRel`：进程与其子进程之间的关系
* `ProcThreadRel`：进程、子进程以及它地址空间内的线程之间的关系


