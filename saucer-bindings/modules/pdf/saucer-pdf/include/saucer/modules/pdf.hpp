#pragma once

#include <saucer/webview.hpp>

#include <utility>
#include <cstdint>
#include <filesystem>

namespace saucer::modules
{
    namespace fs = std::filesystem;

    enum class layout : std::uint8_t
    {
        portrait,
        landscape,
    };

    struct print_settings
    {
        fs::path file;

      public:
        layout orientation{layout::portrait};
        std::pair<double, double> size{8.3, 11.7};
    };

    class pdf
    {
        saucer::webview *m_parent;

      public:
        pdf(saucer::webview *parent);

      public:
        void save(const print_settings &settings);
    };
} // namespace saucer::modules
