/*
 * The C ABI's contract, pinned from C against the generated header:
 * the same checks tests/ffi.rs makes, run against the shared library
 * the way a native binding would link it. Each CHECK prints one line;
 * the program's exit status is the number of failures.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "geist4.h"

static int failures = 0;

#define CHECK(cond, ...)                                                    \
    do {                                                                    \
        if (cond) {                                                         \
            printf("ok - " __VA_ARGS__);                                    \
            printf("\n");                                                   \
        } else {                                                            \
            printf("FAIL - " __VA_ARGS__);                                  \
            printf("\n");                                                   \
            failures++;                                                     \
        }                                                                   \
    } while (0)

/* Whether the len-byte message contains needle (NUL-terminated). */
static int contains(const char *message, size_t len, const char *needle) {
    size_t needle_len = strlen(needle);
    if (needle_len == 0 || len < needle_len) {
        return needle_len == 0;
    }
    for (size_t at = 0; at + needle_len <= len; at++) {
        if (memcmp(message + at, needle, needle_len) == 0) {
            return 1;
        }
    }
    return 0;
}

/* The one-shot render of `data`, read into a static buffer (the tests
 * read and free immediately, so nothing is held across calls). Returns
 * NULL, with the error buffer filled, on failure. */
static const char *render_with(geist4_options *options, const char *data,
                               size_t len, char *err, size_t err_cap,
                               size_t *out_len) {
    static char buffer[4096];
    geist4_string out = {NULL, 0};
    geist4_status status = geist4_render_html(options, data, len, &out, err, err_cap);
    if (status != GEIST4_OK) {
        *out_len = 0;
        return NULL;
    }
    if (out.len >= sizeof(buffer) || strlen(out.data) != out.len) {
        printf("FAIL - output exceeds the test buffer or mismatches len\n");
        failures++;
        geist4_string_free(out);
        *out_len = 0;
        return NULL;
    }
    memcpy(buffer, out.data, out.len);
    buffer[out.len] = '\0';
    *out_len = out.len;
    geist4_string_free(out);
    return buffer;
}

int main(void) {
    char err[256];
    geist4_status status;
    size_t len = 0;
    const char *html;

    /* Version. */
    CHECK(geist4_version() != NULL, "version is non-null");
    CHECK(strlen(geist4_version()) > 0, "version is %s", geist4_version());

    /* Plain parse and render. */
    geist4_options *plain = geist4_options_create();
    CHECK(plain != NULL, "options created");
    err[0] = '\0';
    html = render_with(plain, "# hi\n", 5, err, sizeof(err), &len);
    CHECK(html != NULL, "plain parse renders");
    CHECK(html != NULL && strcmp(html, "<h1>hi</h1>\n") == 0,
          "heading renders byte for byte");

    /* An empty document renders empty, with a non-null pointer. */
    err[0] = '\0';
    html = render_with(plain, "", 0, err, sizeof(err), &len);
    CHECK(html != NULL && len == 0, "empty document renders empty");
    CHECK(html != NULL && html[0] == '\0', "empty output is terminated");

    /* A NUL byte is ordinary content: U+FFFD replaces it. */
    err[0] = '\0';
    html = render_with(plain, "a\0b\n", 4, err, sizeof(err), &len);
    CHECK(html != NULL && len == 13 &&
              strcmp(html, "<p>a\xEF\xBF\xBD" "b</p>\n") == 0,
          "NUL becomes U+FFFD");

    /* Null input is rejected; the handle is null and the buffer filled. */
    err[0] = '\0';
    geist4_document *document =
        geist4_parse(plain, NULL, 5, err, sizeof(err));
    CHECK(document == NULL, "null input parses nothing");
    CHECK(err[0] != '\0' && strstr(err, "null") != NULL,
          "null input reports: %s", err);

    /* Invalid UTF-8 is rejected with the byte offset. */
    err[0] = '\0';
    document = geist4_parse(plain, "a\xff" "b", 3, err, sizeof(err));
    CHECK(document == NULL, "invalid UTF-8 parses nothing");
    CHECK(err[0] != '\0' && strstr(err, "byte 1") != NULL,
          "invalid UTF-8 reports: %s", err);

    /* Invalid UTF-8 through the one-shot path leaves out untouched. */
    {
        geist4_string out = {(char *)0x1, 99};
        err[0] = '\0';
        status = geist4_render_html(NULL, "a\xff" "b", 3, &out, err, sizeof(err));
        CHECK(status == GEIST4_ERR_UTF8, "one-shot rejects invalid UTF-8");
        CHECK(out.data == (char *)0x1 && out.len == 99,
              "out untouched on failure");
    }

    /* The gfm bundle. */
    geist4_options *gfm = geist4_options_create();
    err[0] = '\0';
    status = geist4_options_enable_builtin(gfm, "gfm", 3, err, sizeof(err));
    CHECK(status == GEIST4_OK, "gfm bundle enabled: %s", err);
    err[0] = '\0';
    html = render_with(gfm, "~~gone~~\n", 9, err, sizeof(err), &len);
    CHECK(html != NULL && strcmp(html, "<p><del>gone</del></p>\n") == 0,
          "strikethrough renders");

    err[0] = '\0';
    html = render_with(gfm, "| a | b |\n| --- | --- |\n| 1 | 2 |\n", 34, err,
                       sizeof(err), &len);
    CHECK(html != NULL &&
              strcmp(html,
                     "<table>\n<thead>\n<tr>\n<th>a</th>\n<th>b</th>\n</tr>\n"
                     "</thead>\n<tbody>\n<tr>\n<td>1</td>\n<td>2</td>\n</tr>\n"
                     "</tbody>\n</table>\n") == 0,
          "table renders byte for byte");

    /* An unknown name is rejected with the valid names listed. */
    err[0] = '\0';
    status = geist4_options_enable_builtin(gfm, "nope", 4, err, sizeof(err));
    CHECK(status == GEIST4_ERR_UNKNOWN_EXT, "unknown name rejected");
    CHECK(strstr(err, "valid names") != NULL, "rejection lists names: %s", err);

    /* A document handle: parse, problems, render; the options handle
     * may be freed first because the document keeps its own copy. */
    geist4_options *strike = geist4_options_create();
    err[0] = '\0';
    status = geist4_options_enable_builtin(strike, "strikethrough", 13, err,
                                           sizeof(err));
    CHECK(status == GEIST4_OK, "strikethrough enabled");
    const char *source = "~~x~~ [text][missing]\n";
    document = geist4_parse(strike, source, strlen(source), err, sizeof(err));
    CHECK(document != NULL, "document parsed");
    geist4_options_free(strike);

    geist4_problem_list problems = {NULL, 0};
    err[0] = '\0';
    status = geist4_document_problems(document, &problems, err, sizeof(err));
    CHECK(status == GEIST4_OK, "problems read");
    CHECK(problems.len >= 1, "%zu problems", problems.len);
    CHECK(problems.items != NULL, "problem items non-null");
    {
        size_t source_len = strlen(source);
        int saw_missing = 0;
        for (size_t i = 0; i < problems.len; i++) {
            const geist4_problem *problem = &problems.items[i];
            if (problem->start >= source_len || problem->end > source_len ||
                problem->start >= problem->end) {
                CHECK(0, "problem %zu: span outside the source", i);
                continue;
            }
            if (contains(problem->message, problem->message_len, "missing")) {
                saw_missing = 1;
            }
        }
        CHECK(saw_missing, "a problem names the missing reference");
    }

    {
        geist4_string out = {NULL, 0};
        err[0] = '\0';
        status = geist4_document_html(document, &out, err, sizeof(err));
        CHECK(status == GEIST4_OK, "document renders after options freed");
        CHECK(out.data != NULL && strlen(out.data) == out.len,
              "len agrees with the terminator");
        CHECK(out.data != NULL &&
                  strcmp(out.data, "<p><del>x</del> [text][missing]</p>\n") == 0,
              "document HTML byte for byte");

        /* A second render of the same document. */
        geist4_string again = {NULL, 0};
        err[0] = '\0';
        status = geist4_document_html(document, &again, err, sizeof(err));
        CHECK(status == GEIST4_OK && again.len == out.len,
              "second render identical");
        geist4_string_free(out);
        geist4_string_free(again);
    }
    geist4_document_free(document);
    geist4_options_free(gfm);
    geist4_options_free(plain);

    /* Updates: a document handle parses, updates, reports spans. */
    {
        err[0] = '\0';
        geist4_document *previous =
            geist4_parse(NULL, "one\n\ntwo\n\nthree\n", 17, err, sizeof(err));
        CHECK(previous != NULL, "previous document parsed: %s", err);
        const char *edited = "one\n\nTWO\n\nthree\n";
        err[0] = '\0';
        geist4_document *updated =
            geist4_update(previous, edited, strlen(edited), err, sizeof(err));
        CHECK(updated != NULL, "update succeeded: %s", err);

        geist4_change_list changes = {NULL, 0};
        err[0] = '\0';
        status = geist4_document_spans(updated, &changes, err, sizeof(err));
        CHECK(status == GEIST4_OK, "spans read: %s", err);
        CHECK(changes.len == 1, "one change, %zu reported", changes.len);
        int covered = changes.len == 1 &&
                      changes.items[0].start <= 6 &&
                      changes.items[0].end >= 9;
        CHECK(covered, "the change covers the edited bytes");

        /* The update's HTML equals a fresh parse's HTML. */
        geist4_string from_update = {NULL, 0};
        status = geist4_document_html(updated, &from_update, err, sizeof(err));
        CHECK(status == GEIST4_OK, "updated document renders");
        geist4_document *full =
            geist4_parse(NULL, edited, strlen(edited), err, sizeof(err));
        CHECK(full != NULL, "oracle parsed");
        geist4_string from_full = {NULL, 0};
        status = geist4_document_html(full, &from_full, err, sizeof(err));
        CHECK(status == GEIST4_OK, "oracle renders");
        CHECK(from_update.len == from_full.len &&
                  memcmp(from_update.data, from_full.data, from_full.len) == 0,
              "update matches a full parse");

        /* The previous document stays usable until freed, and freeing
         * it does not disturb the update's handle. */
        geist4_string before = {NULL, 0};
        status = geist4_document_html(previous, &before, err, sizeof(err));
        CHECK(status == GEIST4_OK, "previous document still renders");
        geist4_document *after =
            geist4_update(previous, edited, strlen(edited), err, sizeof(err));
        CHECK(after != NULL, "the previous document updates again");
        geist4_document_free(after);
        geist4_document_free(previous);
        geist4_string_free(before);

        /* An identical source reports nothing. */
        err[0] = '\0';
        geist4_document *same =
            geist4_update(updated, edited, strlen(edited), err, sizeof(err));
        CHECK(same != NULL, "identical update succeeds");
        geist4_change_list none = {NULL, 0};
        status = geist4_document_spans(same, &none, err, sizeof(err));
        CHECK(status == GEIST4_OK && none.len == 0,
              "identical sources report no changes");

        /* A null previous handle is an error and leaves the buffer
         * filled. */
        err[0] = '\0';
        geist4_document *bad =
            geist4_update(NULL, edited, strlen(edited), err, sizeof(err));
        CHECK(bad == NULL, "null previous rejected");
        CHECK(strstr(err, "null") != NULL, "null previous reports: %s", err);

        geist4_string_free(from_update);
        geist4_string_free(from_full);
        geist4_document_free(full);
        geist4_document_free(updated);
        geist4_document_free(same);
    }

    /* Null frees are no-ops. */
    geist4_document_free(NULL);
    geist4_options_free(NULL);
    {
        geist4_string out = {NULL, 0};
        geist4_string_free(out);
    }
    CHECK(1, "null frees are no-ops");

    if (failures == 0) {
        printf("all C checks passed\n");
        return 0;
    }
    printf("%d check(s) failed\n", failures);
    return 1;
}
