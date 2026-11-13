/* Links librotulus-1 the way an application does, through the installed
 * header, and drives both row APIs. */

#include <rotulus.h>

static GdkPaintable *
avatar (RotulusView *view, guint64 key, gpointer user_data)
{
    (void) view;
    (void) user_data;
    if (key != 42)
        return NULL;
    /* A full reference, per RotulusAvatarFunc. */
    return GDK_PAINTABLE (gdk_paintable_new_empty (16, 16));
}

static int destroyed;

static void
on_destroy (gpointer data)
{
    (void) data;
    destroyed++;
}

int
main (void)
{
    gtk_init ();

    GtkWidget *widget = rotulus_view_new ();
    g_assert_true (ROTULUS_IS_VIEW (widget));
    g_object_ref_sink (widget);
    RotulusView *view = ROTULUS_VIEW (widget);

    rotulus_view_set_avatar_func (view, avatar, NULL, on_destroy);

    RotulusRun nick = ROTULUS_RUN_PLAIN ("alice", -1);
    RotulusRun body
        = ROTULUS_RUN ("hello", -1, ROTULUS_COLOR_DEFAULT, ROTULUS_ATTR_BOLD);
    RotulusRow row = {
        .kind = ROTULUS_ROW_MESSAGE,
        .speaker = { .key = 42, .nick = "alice", .nick_len = -1 },
        .gutter = &nick,
        .n_gutter = 1,
        .body = &body,
        .n_body = 1,
    };
    RotulusMark *first = rotulus_view_append (view, &row);
    g_assert_nonnull (first);

    g_autoptr (RotulusMessage) msg = rotulus_message_new (ROTULUS_ROW_MESSAGE);
    rotulus_message_set_speaker (msg, 7, "bob");
    rotulus_message_add_text (msg, "see https://example.com", ROTULUS_COLOR_DEFAULT,
                              ROTULUS_ATTR_NONE);
    rotulus_message_add_mirc (msg, " \x02" "bold\x02 and \x03" "4red");
    RotulusMark *second = rotulus_view_append_message (view, msg);
    g_assert_nonnull (second);
    g_assert_true (rotulus_view_get_last (view) == second);

    g_autoptr (RotulusMessage) copy = rotulus_message_copy (msg);
    g_assert_true (rotulus_view_replace_message (view, first, copy));

    /* Both rows say "bold" now. */
    guint n = 0, current = 0;
    rotulus_view_search (view, "bold", FALSE, &n, &current);
    g_assert_cmpuint (n, ==, 2);
    g_assert_cmpuint (current, >=, 1);

    /* A reply header, set after its row, changed, and taken away. */
    g_assert_true (rotulus_view_set_reply (view, second, "alice",
                                           ROTULUS_COLOR_DEFAULT, "hello"));
    g_assert_true (rotulus_view_set_reply_rgb (view, second, "alice", 0x26a269,
                                               "hello"));
    g_assert_false (rotulus_view_set_reply (view, second, NULL,
                                            ROTULUS_COLOR_DEFAULT, NULL));
    g_assert_true (rotulus_view_set_reply_unavailable (view, second, "gone"));
    g_assert_true (rotulus_view_clear_reply (view, second));

    g_autoptr (RotulusReactions) reactions = rotulus_reactions_new ();
    rotulus_reactions_add (reactions, "+1", 3, TRUE, "thumbs up");
    rotulus_reactions_add_full (reactions, "emoji:17", "?", 1, FALSE, NULL);
    g_assert_true (rotulus_view_set_reactions (view, second, reactions));
    g_assert_true (rotulus_view_set_reactions (view, second, NULL));

    GdkRGBA nick_color = { 0 };
    rotulus_view_get_text_color (view, ROTULUS_PAL_NICK_COLOR0, &nick_color);
    g_assert_cmpfloat (nick_color.alpha, >, 0.0);

    g_assert_true (rotulus_view_reveal (view, first, FALSE));
    rotulus_view_set_spotlight (view, first);
    rotulus_view_set_spotlight (view, NULL);

    /* Never given a size, it has no viewport. */
    RotulusMark *top = second, *bottom = second;
    g_assert_false (rotulus_view_get_visible_rows (view, &top, &bottom));
    g_assert_true (top == NULL && bottom == NULL);
    g_assert_false (rotulus_view_is_row_visible (view, second));
    g_assert_null (rotulus_view_get_focus_row (view));

    g_assert_true (rotulus_view_remove (view, first));
    g_assert_false (rotulus_view_remove (view, first));
    g_assert_false (rotulus_view_reveal (view, first, TRUE));
    g_assert_false (rotulus_view_set_reply (view, first, "alice",
                                            ROTULUS_COLOR_DEFAULT, "hello"));

    int n_runs = 0;
    RotulusRun *runs = rotulus_mirc_parse ("\x02" "a\x02" "b", -1, &n_runs);
    g_assert_cmpint (n_runs, ==, 2);
    g_free (runs);

    rotulus_view_set_avatar_func (view, NULL, NULL, NULL);
    g_assert_cmpint (destroyed, ==, 1);

    g_object_unref (widget);
    return 0;
}
