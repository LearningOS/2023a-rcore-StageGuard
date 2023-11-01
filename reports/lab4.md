# 简述功能

## `sys_linkat`, `sys_unlinkat` 和 `sys_stat` 实现

在 easy-fs 中的 `INode` 中添加方法：

* `pub fn link_at(&self, src: &str, dst: &str) -> Option<Arc<Inode>>`

将 `src` 硬链接到 `dst` 文件，先获取被链接的文件 inode_id，然后在 `DiskInode` 中写入一个 `DirEntry`，文件名为 `dst`，inode_id 和 `src` 相同。

然后返回一个新的指向 `src` inode 的 `Inode`（也是指向 `dst` 的，因为他们 `inode_id` 一样）。

* `pub fn unlink_at(&self, name: &str) -> bool`

解除文件名为 `name` 的硬链接，操作对象 self 必须是目录才行，意思就是解除这个目录下文件名 `name` 的硬链接。

解除之前会先获取这个链接指向的文件索引的所有硬链接，如果只有一个硬链接（也就是文件 `name`），那还要顺便删除这个文件 `DiskInode`。

判断完之后再从目录里删除链接。

* `pub fn stat(&self) -> (u32, usize, u8)`

从根目录遍历一遍所有文件，找到和 self 的 block_id 和 block_offset 相同的文件 `DirEnt`，再获取到他的 `inode_id`。

通过这个 `inode_id` 来找硬链接数，文件类型。

# 简答作业

## 在我们的easy-fs中，root inode起着什么作用？如果root inode中的内容损坏了，会发生什么？

root inode 现在指的就是根目录，如果这个损坏了就没办法索引根目录的文件，进而无法索引所有子目录文件。

# 荣誉准则

1. 在完成本次实验的过程（含此前学习的过程）中，我未与其他人交流本次实验相关的内容。
2. 此外，参考了 **以下资料** ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

* RISC-V Assembler Reference: https://michaeljclark.github.io/asm.html
* 《RISC-V 手册》: http://riscvbook.com/chinese/RISC-V-Reader-Chinese-v2p1.pdf
* Control and Status Registers (CSRs): https://five-embeddev.com/quickref/csrs.html

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。