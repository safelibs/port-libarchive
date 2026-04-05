#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>

struct archive;

extern void archive_set_error_message(struct archive *a, int error_number, const char *message);

void archive_variadic_shim_link_anchor(void) {}

void archive_set_error(struct archive *a, int error_number, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);

    char stack_buffer[1024];
    va_list ap_copy;
    va_copy(ap_copy, ap);
    int needed = vsnprintf(stack_buffer, sizeof(stack_buffer), fmt, ap_copy);
    va_end(ap_copy);

    if (needed < 0) {
        archive_set_error_message(a, error_number, "");
        va_end(ap);
        return;
    }

    if ((size_t)needed < sizeof(stack_buffer)) {
        archive_set_error_message(a, error_number, stack_buffer);
        va_end(ap);
        return;
    }

    char *heap_buffer = (char *)malloc((size_t)needed + 1);
    if (heap_buffer == NULL) {
        archive_set_error_message(a, error_number, "");
        va_end(ap);
        return;
    }

    vsnprintf(heap_buffer, (size_t)needed + 1, fmt, ap);
    archive_set_error_message(a, error_number, heap_buffer);
    free(heap_buffer);
    va_end(ap);
}
