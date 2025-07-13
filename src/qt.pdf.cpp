#include "pdf.hpp"

#include "qt.webview.impl.hpp"

#include <atomic>

namespace saucer::modules
{
    void pdf::save(const print_settings &settings)
    {
        if (!m_parent->parent().thread_safe())
        {
            return m_parent->parent().dispatch([this, settings] { return save(settings); });
        }

        auto &webview = m_parent->native<false>()->web_view;
        auto *page    = webview->page();

        auto [width, height] = settings.size;

        auto page_size   = QPageSize{{width, height}, QPageSize::Unit::Inch};
        auto orientation = settings.orientation == layout::landscape ? QPageLayout::Orientation::Landscape
                                                                     : QPageLayout::Orientation::Portrait;

        std::error_code ec{};

        if (auto parent = settings.file.parent_path(); !fs::exists(parent))
        {
            fs::create_directories(parent, ec);
        }

        auto path   = fs::weakly_canonical(settings.file, ec);
        auto layout = QPageLayout{page_size, orientation, QMarginsF{}};

        page->printToPdf(QString::fromStdString(path.string()), layout);

        std::atomic_bool finished{false};
        page->connect(page, &QWebEnginePage::pdfPrintingFinished, [&finished]() { finished.store(true); });

        while (!finished)
        {
            m_parent->parent().run<false>();
        }
    }
} // namespace saucer::modules
