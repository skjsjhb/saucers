#include "wk.pdf.impl.hpp"

#include "wk.webview.impl.hpp"

#include "cocoa.utils.hpp"
#include "cocoa.window.impl.hpp"

#include <atomic>

namespace saucer::modules
{
    void pdf::save(const print_settings &settings)
    {
        const utils::autorelease_guard guard{};

        if (!m_parent->parent().thread_safe())
        {
            return m_parent->parent().dispatch([this, settings] { return save(settings); });
        }

        auto &webview = m_parent->native<false>()->web_view;
        auto *window  = m_parent->window::native<false>()->window;

        auto *const info = [NSPrintInfo sharedPrintInfo];

        info.paperSize   = NSMakeSize(settings.size.first * 72, settings.size.second * 72);
        info.orientation = settings.orientation == layout::landscape //
                               ? NSPaperOrientationLandscape
                               : NSPaperOrientationPortrait;

        std::error_code ec{};

        if (auto parent = settings.file.parent_path(); !fs::exists(parent))
        {
            fs::create_directories(parent, ec);
        }

        auto path       = fs::weakly_canonical(settings.file, ec);
        auto *const url = [NSURL fileURLWithPath:[NSString stringWithUTF8String:path.c_str()]];

        [info.dictionary setValue:NSPrintSaveJob forKey:NSPrintJobDisposition];
        [info.dictionary setValue:url forKey:NSPrintJobSavingURL];

        auto *const operation = [webview.get() printOperationWithPrintInfo:info];

        operation.showsPrintPanel    = false;
        operation.showsProgressPanel = false;

        operation.view.frame = NSMakeRect(0, 0, info.paperSize.width, info.paperSize.height);

        std::atomic_bool finished{false};

        auto *const delegate = [[[PrintDelegate alloc] initWithCallback:[&]
                                                       {
                                                           finished.store(true);
                                                       }] autorelease];

        [operation runOperationModalForWindow:window
                                     delegate:delegate
                               didRunSelector:@selector(printOperationDidRun:success:contextInfo:)
                                  contextInfo:nullptr];

        while (!finished)
        {
            m_parent->parent().run<false>();
        }
    }
} // namespace saucer::modules
