#pragma once

#include "memory.h"

#include <string>

namespace bindings {
    inline char *alloc(const std::string &value) {
        const auto size = value.size();
        char *const rtn = static_cast<char *>(saucer_memory_alloc(size + 1));

        value.copy(rtn, size);
        rtn[size] = '\0';

        return rtn;
    }

    inline char8_t *alloc(const std::basic_string<char8_t> &value) {
        const auto size = value.size();
        char8_t *const rtn = static_cast<char8_t *>(saucer_memory_alloc(size + 1));

        value.copy(rtn, size);
        rtn[size] = '\0';

        return rtn;
    }
} // namespace bindings
