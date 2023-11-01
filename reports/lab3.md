# 简述功能

## `sys_spawn` 实现

在 `impl TaskControlBlock` 里复制一份 `new` 方法，创建的 TCB 中 `parent` 设置为 `Some(Arc::downgrade(self))`，然后再把这个 TCB push 到 parent 的 children 向量里。

## stride 调度和 `sys_set_priority` 实现

在 `TaskControlBlockInner` 里添加 `stride: usize` 和 `priority: usize` 字段，然后在 `impl TaskManager` 里 添加 `fetch_with_min_stride` 方法获取 `stride` 最小的 TCB。

修改 `run_tasks` 方法，使用 `fetch_with_min_stride` 获取 TCB，然后添加 `task_inner.stride += BIG_STRIDE / task_inner.priority;` 增加当前调度进程的 stride。


# 简答作业

先摆了

# 荣誉准则

1. 在完成本次实验的过程（含此前学习的过程）中，我未与其他人交流本次实验相关的内容。
2. 此外，参考了 **以下资料** ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

* RISC-V Assembler Reference: https://michaeljclark.github.io/asm.html
* 《RISC-V 手册》: http://riscvbook.com/chinese/RISC-V-Reader-Chinese-v2p1.pdf
* Control and Status Registers (CSRs): https://five-embeddev.com/quickref/csrs.html

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。
