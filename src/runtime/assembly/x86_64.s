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

// fn(rdi: sp, rsi: func) -> rax: sp
.global prepare_stack
prepare_stack:
    mov rax, rdi                        // The first argument of make_fcontext() == top of context-stack
    and rax, -16                        // Shift address in RAX to lower 16-byte boundary
    lea rax, [rax - 0x40]               // Reserve space for context-data on context-stack

    stmxcsr [rax]                       // Save MMX control-word and status-word
    fnstcw  [rax + 0x04]                // Save x87 control-word
    mov [rax + 0x28], rsi               // 2-rd arg of make_fcontext() == address of context-fn, store in RBX

    lea rcx, [rip + trampoline]         // Compute absolute address of label trampoline
    mov [rax + 0x38], rcx               // Save the addr of trampoline as a return-address for context-fn
                                        // will be entered after the context-function returns
    ret                                 // Return pointer to context-data

.global trampoline
trampoline:
    push rbp                            // Store return address on stack, fix stack alignment
    // set rdi == clo_data
    // set rsi == stack_ptr
    jmp rbx                             // Jump to context-function


.global jump
jump:
    lea rsp, [rsp - 0x38]               // Prepare stack (RIP is already stored in stack)

    stmxcsr [rsp]                       // Save MMX control-word and status-word
    fnstcw  [rsp + 0x04]                // Save x87 control-word
    mov [rsp + 0x08], r12
    mov [rsp + 0x10], r13
    mov [rsp + 0x18], r14
    mov [rsp + 0x20], r15
    mov [rsp + 0x28], rbx
    mov [rsp + 0x30], rbp               // Save base poiter

    mov [rsi], rsp                      // Save SP (pointing to context-data) to the secong arg ptr
    mov rsp, rdi                        // Restore SP (pointing to context-data) from RDI
    mov r8, [rsp + 0x38]                // Restore return-address

    ldmxcsr [rsp]                       // Restore MMX control-word and status-word
    fldcw   [rsp + 0x04]                // Restore x87 control-word
    mov r12, [rsp + 0x08]
    mov r13, [rsp + 0x10]
    mov r14, [rsp + 0x18]
    mov r15, [rsp + 0x20]
    mov rbx, [rsp + 0x28]
    mov rbp, [rsp + 0x30]               // Restore base poiter
    lea rsp, [rsp + 0x40]               // Clear stack

    jmp r8                              // Jump to context fn
