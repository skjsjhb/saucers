#include "pdf.hpp"

#include "wkg.webview.impl.hpp"
#include "handle.hpp"

#include <atomic>

namespace saucer::modules
{
    using paper_size_handle = utils::handle<GtkPaperSize *, gtk_paper_size_free>;

    void pdf::save(const print_settings &settings)
    {
        if (!m_parent->parent().thread_safe())
        {
            return m_parent->parent().dispatch([this, settings] { return save(settings); });
        }

        auto *webview = m_parent->native<false>()->web_view;

        auto operation      = utils::g_object_ptr<WebKitPrintOperation>{webkit_print_operation_new(webview)};
        auto print_settings = utils::g_object_ptr<GtkPrintSettings>{gtk_print_settings_new()};

        gtk_print_settings_set_printer(print_settings.get(), "Print to File");
        gtk_print_settings_set(print_settings.get(), GTK_PRINT_SETTINGS_OUTPUT_FILE_FORMAT, "pdf");

        std::error_code ec{};

        if (auto parent = settings.file.parent_path(); !fs::exists(parent))
        {
            fs::create_directories(parent, ec);
        }

        const auto parent   = fs::weakly_canonical(settings.file.parent_path(), ec);
        const auto filename = settings.file.filename().replace_extension();

        gtk_print_settings_set(print_settings.get(), GTK_PRINT_SETTINGS_OUTPUT_DIR, parent.c_str());
        gtk_print_settings_set(print_settings.get(), GTK_PRINT_SETTINGS_OUTPUT_BASENAME, filename.c_str());

        webkit_print_operation_set_print_settings(operation.get(), print_settings.get());

        auto [width, height] = settings.size;
        auto paper_size      = paper_size_handle{gtk_paper_size_new_custom("", "", width, height, GTK_UNIT_INCH)};
        auto setup           = utils::g_object_ptr<GtkPageSetup>{gtk_page_setup_new()};

        gtk_page_setup_set_top_margin(setup.get(), 0, GTK_UNIT_INCH);
        gtk_page_setup_set_bottom_margin(setup.get(), 0, GTK_UNIT_INCH);

        gtk_page_setup_set_left_margin(setup.get(), 0, GTK_UNIT_INCH);
        gtk_page_setup_set_right_margin(setup.get(), 0, GTK_UNIT_INCH);

        gtk_page_setup_set_paper_size(setup.get(), paper_size.get());
        webkit_print_operation_set_page_setup(operation.get(), setup.get());

        gtk_page_setup_set_orientation(setup.get(), settings.orientation == layout::landscape
                                                        ? GTK_PAGE_ORIENTATION_LANDSCAPE
                                                        : GTK_PAGE_ORIENTATION_PORTRAIT);

        std::atomic_bool finished{false};

        auto callback = [](void *, std::atomic_bool *finished)
        {
            finished->store(true);
        };

        g_signal_connect(operation.get(), "finished", G_CALLBACK(+callback), &finished);
        webkit_print_operation_print(operation.get());

        while (!finished)
        {
            m_parent->parent().run<false>();
        }
    }
} // namespace saucer::modules
