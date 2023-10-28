/******************************************************************************
 * 0x38 : RIP - return address          = &trampoline
 * 0x30 : RBP - base pointer            = &finish // removed
 * 0x28 : RBX - function pointer        = &fn()
 * 0x20 : R15
 * 0x18 : R14
 * 0x10 : R13                           rbx : used in trampoline, set before call
 * 0x08 : R12
 * 0x04 : fc_x87_cw - 4B                rsp : stack pointer
 * 0x00 : fc_mxcsr  - 4B                rip : instruction pointer (r/o)
 ******************************************************************************/

.global prepare_stack                   // fn(rdi: stack, rsi: func) -> rax: continuation
prepare_stack:
    mov rax, rdi                        // The first argument of prepare_stack() == top of context-stack
    and rax, -16                        // Shift address in RAX to lower 16-byte boundary
    lea rax, [rax - 0x40]               // Reserve space for context-data on context-stack

    stmxcsr [rax]                       // Save MMX control-word and status-word
    fnstcw  [rax + 0x04]                // Save x87 control-word
    mov [rax + 0x28], rsi               // 2-rd arg of prepare_stack() == address of context-fn, store in RBX

    lea rcx, [rip + trampoline]         // Compute absolute address of label trampoline
    mov [rax + 0x38], rcx               // Save the addr of trampoline as a return-address for func will be entered after the context-function returns
    ret                                 // Return pointer to context-data

.global trampoline
trampoline:
    push rbp                            // Store return address on stack, fix stack alignment
    jmp rbx                             // Jump to context-function


.global jump                            // fn (rdi: from, rsi: to)
jump:
    lea rsp, [rsp - 0x38]               // Prepare stack (RIP is already stored in stack)

    stmxcsr [rsp]                       // Save MMX control-word and status-word
    fnstcw  [rsp + 0x04]                // Save x87 control-word
    mov [rsp + 0x08], r12
    mov [rsp + 0x10], r13
    mov [rsp + 0x18], r14
    mov [rsp + 0x20], r15
    mov [rsp + 0x28], rbx
    mov [rsp + 0x30], rbp

    mov [rdi], rsp                      // Save SP (pointing to context-data) to the first arg (RDI)
    mov rsp, [rsi]                      // Restore SP (pointing to context-data) from second arg (RSI)

    ldmxcsr [rsp]                       // Restore MMX control-word and status-word
    fldcw   [rsp + 0x04]                // Restore x87 control-word
    mov r12, [rsp + 0x08]
    mov r13, [rsp + 0x10]
    mov r14, [rsp + 0x18]
    mov r15, [rsp + 0x20]
    mov rbx, [rsp + 0x28]
    mov rbp, [rsp + 0x30]
    lea rsp, [rsp + 0x38]               // Clear stack

    ret                                 // Jump to the address at [rsp]
