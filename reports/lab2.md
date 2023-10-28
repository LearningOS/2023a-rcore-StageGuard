# 简述功能

## 重新实现 `sys_get_time` 和 `sys_task_info`

引入虚拟内存后，现在传入这两个 syscall 的指针都是用户空间的地址指针，需要查询页表定位到监管模式下的虚拟内存的物理地址。

在看到 `sys_write` 改进实现后，我参考了 `translated_byte_buffer` 的实现，自己实现了一个类似功能的函数：

```rust
pub fn translate_user_space_ptr<T>(token: usize, ptr: *const T, size: usize) -> &'static mut T
```

该函数可以根据用户空间的应用 `token`，将这个应用空间的 `ptr` 转换为对应的物理页帧中实际地址。

原理就是通过 `token` 构建用于查找的临时 `PageTable`，使用 `PageTable.translate` 找到对应的物理页号，获取这一页的 `&mut [u8]` 引用。

再让 `ptr` 对齐页大小后多出来的偏移量作为下标，就能得到一个 `&mut u8`，这个 u8 引用就是 `ptr` 所指对象应该放的位置。

但是对于大于页大小的对象，则需要分页存储，目前还未实现，我认为同样也可以参考 `translated_byte_buffer` 来实现。

## `sys_mmap` 和 `sys_munmap` 实现

在 `TaskControlBlock` 中新增两个函数用于 mmap 和 munmap。

mmap 根据传入的 `start` 和 `len` 计算出向下取整的 `start_virtual_address` 和 向上取整的 `end_virtual_address`。

先转换成 `VPNRange` 与 `MemorySet.areas` 里的每一个逻辑块检测一下是否有交集。

没有交集就证明范围合法，再调用 `insert_framed_area` 插入即可。

`munmap` 同理，检测 `areas` 里是否有传入的 `VPNRange` 逻辑块，有就删掉没有就删除失败。


# 简答作业

## 请列举 SV39 页表页表项的组成，描述其中的标志位有何作用

从低位到高位：

* 0: Valid，页表是否合法。
* 1-3: Read/Write/eXecute，表示页表内容是否可读/写/执行。
* 4: User，表示在用户特权级下是否可以访问。
* 6: Accessed：处理器记录自从页表项上的这一位被清零之后，页表项的对应虚拟页面是否被访问过；
* 7: Dirty：处理器记录自从页表项上的这一位被清零之后，页表项的对应虚拟页面是否被修改过。
* 8-9: 保留字
* 10-53: 物理页号
* 54-63: 保留字

## 缺页

* 缺页会以 LoadPageFault 和 LoadFalut trap 到监管特权级，根据操作系统的调度将导致当前程序暂停执行，暂停时间取决于操作系统的缺页处理算法和能力。
* 发生缺页时，`stval` 寄存器指向了无效的用户空间地址。

## 双页表与单页表

* **在单页表情况下，如何更换页表？**

摆了

* 单页表情况下，如何控制用户态无法访问内核页面？

设置 SV39 的 U 位为 0

* 单页表有何优势？（回答合理即可）

可能可以节省内存空间

* 双页表实现下，何时需要更换页表？假设你写一个单页表操作系统，你会选择何时更换页表（回答合理即可）？

进入 trap 和 restore

# 荣誉准则

1. 在完成本次实验的过程（含此前学习的过程）中，我未与其他人交流本次实验相关的内容。
2. 此外，参考了 **以下资料** ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

* RISC-V Assembler Reference: https://michaeljclark.github.io/asm.html
* 《RISC-V 手册》: http://riscvbook.com/chinese/RISC-V-Reader-Chinese-v2p1.pdf
* Control and Status Registers (CSRs): https://five-embeddev.com/quickref/csrs.html

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。