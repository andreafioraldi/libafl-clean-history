#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#define MAP_SIZE (16*1024)

#ifdef _WIN32
#define posix_memalign(p, a, s) (((*(p)) = _aligned_malloc((s), (a))), *(p) ?0 :errno)
#endif

#define MAX(a, b)           \
  ({                        \
                            \
    __typeof__(a) _a = (a); \
    __typeof__(b) _b = (b); \
    _a > _b ? _a : _b;      \
                            \
  })

size_t libafl_alloc_map[MAP_SIZE];

void *malloc(size_t size) {

  uintptr_t k = (uintptr_t)__builtin_return_address(0);
  k = (k >> 4) ^ (k << 8);
  k &= MAP_SIZE - 1;
  libafl_alloc_map[k] = MAX(libafl_alloc_map[k], size);

  // We cannot malloc in malloc.
  // Hence, even realloc(NULL, size) would loop in an optimized build.
  // We fall back to a stricter allocation function. Fingers crossed.
  void *ret = NULL;
  posix_memalign(&ret, 1<<6, size);
  return ret;

}

void *calloc(size_t nmemb, size_t size) {

  size *= nmemb;

  uintptr_t k = (uintptr_t)__builtin_return_address(0);
  k = (k >> 4) ^ (k << 8);
  k &= MAP_SIZE - 1;
  libafl_alloc_map[k] = MAX(libafl_alloc_map[k], size);

  void *ret = NULL;
  posix_memalign(&ret, 1<<6, size);
  memset(ret, 0, size);
  return ret;

}
