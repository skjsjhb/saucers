project(saucer-bindings-pdf LANGUAGES CXX VERSION 5.0.0)

# --------------------------------------------------------------------------------------------------------
# Setup library
# --------------------------------------------------------------------------------------------------------

if (SAUCERS_SHARED_LIB)
    add_library(${PROJECT_NAME} SHARED)
else ()
    add_library(${PROJECT_NAME} STATIC)
endif ()
add_library(saucer::bindings::pdf ALIAS ${PROJECT_NAME})

target_compile_features(${PROJECT_NAME} PRIVATE cxx_std_23)
set_target_properties(${PROJECT_NAME} PROPERTIES CXX_STANDARD 23 CXX_EXTENSIONS OFF CXX_STANDARD_REQUIRED ON)

# --------------------------------------------------------------------------------------------------------
# Include directories
# --------------------------------------------------------------------------------------------------------

target_include_directories(${PROJECT_NAME} PUBLIC  "include")
target_include_directories(${PROJECT_NAME} PRIVATE "include/saucer")

# --------------------------------------------------------------------------------------------------------
# Add Sources
# --------------------------------------------------------------------------------------------------------

target_sources(${PROJECT_NAME} PRIVATE 
    "src/pdf.cpp"
)

# --------------------------------------------------------------------------------------------------------
# Setup Dependencies
# --------------------------------------------------------------------------------------------------------

CPMFindPackage(
    NAME           saucer-pdf
    VERSION        1.0.1
    GIT_REPOSITORY "https://github.com/saucer/pdf"
)

target_link_libraries(${PROJECT_NAME} PRIVATE saucer::pdf)
saucer_bindings_add_module(${PROJECT_NAME} "SAUCER_PDF_EXPORT")

cmake_policy(SET CMP0177 NEW)
install(
        TARGETS ${PROJECT_NAME}
        ARCHIVE DESTINATION "${MODULES_INSTALL_DIR}/lib"
        LIBRARY DESTINATION "${MODULES_INSTALL_DIR}"
        RUNTIME DESTINATION "${MODULES_INSTALL_DIR}"
)