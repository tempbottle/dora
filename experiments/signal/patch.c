#include <stdio.h>
#include <stdint.h>
#include <stdbool.h>
#include <signal.h>
#include <unistd.h>
#include <sys/mman.h>
#include <string.h>
#include <ucontext.h>

#define REG_RBP 10
#define REG_RSP 15
#define REG_RIP 16

typedef int (*ftype)();
bool debug = false;

ftype alloc_code(const char *code, size_t len, bool debug_me) {
  long pagesize;

  if ((pagesize = sysconf(_SC_PAGESIZE)) == -1) {
    perror("sysconf(_SC_PAGESIZE) failed");
  }

  void *ptr = mmap(NULL, pagesize, PROT_READ | PROT_WRITE | PROT_EXEC,
    MAP_ANON | MAP_PRIVATE, -1, 0);

  if (debug && debug_me) {
    *((uint8_t *) ptr) = 0xCC;
    memcpy(ptr + 1, code, len);
  } else {
    memcpy(ptr, code, len);
  }

  return ptr;
}

void dump(const char *name, void *ptr, size_t len) {
  uint8_t *byte_ptr = (uint8_t *) ptr;
  printf("%s @ %p (%d bytes) = ", name, byte_ptr, len);

  for(size_t i=0; i<len; i++) {
    printf("%02x ", byte_ptr[i]);
  }

  printf("\n");
}

void handler(int signo, siginfo_t *info, void *context) {
  ucontext_t *ucontext = context;
  mcontext_t *mcontext = &ucontext->uc_mcontext;

  printf("SIGNAL!\n", signo);

  uintptr_t rip = mcontext->gregs[REG_RIP];
  dump("\trip", (void *) rip, 8);

  uintptr_t rbp = mcontext->gregs[REG_RBP];
  printf("\trbp = %p\n", rbp);

  uintptr_t rsp = mcontext->gregs[REG_RSP];
  printf("\trsp = %p\n", rsp);

  uintptr_t *rap = (uintptr_t *) rsp;
  printf("\treturn address = %p\n", *rap);

  uintptr_t ra = *rap - 12;
  printf("\treturn address before call %p\n", ra);

  // push rbp
  // mov eax, 4
  // pop rbp
  // ret
  unsigned char fct2_code[] = {
    0x55,
    0xB8, 4, 0, 0, 0,
    0x5D,
    0xC3
  };

  ftype fct2 = alloc_code(fct2_code, sizeof(fct2_code), false);
  dump("\tfct2", fct2, sizeof(fct2_code));

  // patch function invocation
  uintptr_t *patcher = (uintptr_t *) (ra + 2);
  *patcher = (uintptr_t) fct2;

  dump("\tfct1 patched", (void*) (ra-1), 15);

  // undo function invocation, set RIP before call
  mcontext->gregs[REG_RIP] = (greg_t) ra;

  // remove return address from stack
  // (was pushed by call)
  mcontext->gregs[REG_RSP] = (greg_t) (rsp + 8);
}

int main(int argc, char *argv[]) {
  if (argc > 1 && strcmp(argv[1], "--debug") == 0) {
    debug = true;
  }

  struct sigaction sa;

  sa.sa_sigaction = handler;
  sigemptyset(&sa.sa_mask);
  sa.sa_flags = SA_SIGINFO;

  if (sigaction(SIGSEGV, &sa, NULL) == -1) {
    perror("sigaction failed");
  }

  // int fct2 { return 4; }
  // int fct2_stub { <FAIL> };
  // int fct1() { return fct2(); }

  // compiler stub: mov r10, [9]
  unsigned char fct2_stub[] = { 0x4C, 0x8B, 0x14, 0x25, 9, 0, 0, 0 };
  ftype fct2 = alloc_code(fct2_stub, sizeof(fct2_stub), true);

  dump("fct2_stub", fct2, sizeof(fct2_stub));

  // (int3)
  // push rbp
  // movabs rax, 0x1122334455667788
  // call rax
  // pop rbp
  // ret
  uint8_t fct1_code[] = {
    0x55,
    0x48, 0xB8, 0, 0, 0, 0, 0, 0, 0, 0,
    0xFF, 0xD0,
    0x5D,
    0xC3
  };

  intptr_t *fct1_addr = (intptr_t *) (fct1_code + 3);
  *(fct1_addr) = (intptr_t) fct2;

  ftype fct1 = alloc_code(fct1_code, sizeof(fct1_code), false);
  dump("fct1", fct1, sizeof(fct1_code));

  printf("invoke fct1:\n");
  int res1 = fct1();
  printf("res = %d\n", res1);

  // invoke fct a second time - stub should not be used anymore
  printf("\ninvoke fct1 again:\n");
  int res2 = fct1();
  printf("res = %d\n", res2);

  return 0;
}
