/* SPDX-License-Identifier: GPL-3.0-or-later */

/*
 * Memory layout for AArch64, QEMU 'virt'.
 *
 * The machine places RAM at 0x4000_0000 and QEMU's `-kernel` loads an ELF at the
 * physical addresses its segments declare, so the load address here is also the
 * address the image runs at. There is no MMU yet: physical and virtual are the
 * same thing for the whole of this file's lifetime, and that stops being true
 * the moment paging lands.
 */

ENTRY(_start)

SECTIONS
{
    . = 0x40000000;

    /*
     * .text.boot must come first and must not be discarded: it holds the ELF
     * entry point, and nothing in Rust references it, so KEEP is what stops the
     * linker from garbage-collecting the whole kernel's front door.
     */
    .text : ALIGN(4)
    {
        KEEP(*(.text.boot))
        *(.text .text.*)
    }

    .rodata : ALIGN(8)
    {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(8)
    {
        *(.data .data.*)
    }

    /*
     * The boot stub zeroes this range eight bytes at a time, so both bounds must
     * be 8-byte aligned. Getting that wrong would silently walk past __bss_end.
     */
    .bss : ALIGN(8)
    {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(8);
        __bss_end = .;
    }

    /* Boot stack. Grows downwards from __stack_top; 64 KiB is generous for a
     * pre-scheduler kernel and will be replaced by per-thread stacks. */
    . = ALIGN(16);
    __stack_bottom = .;
    . += 0x10000;
    __stack_top = .;

    __kernel_end = .;

    /* panic = abort comes from the target spec, so unwind tables are dead
     * weight; the notes and comments are of no use to a bare-metal image. */
    /DISCARD/ :
    {
        *(.comment)
        *(.note .note.*)
        *(.eh_frame .eh_frame_hdr)
    }
}
