//! A PDF written by hand, for the tests of everything that reads one.
//!
//! Not a checked-in fixture, and deliberately not. `hayro_syntax` finds its
//! objects by seeking to the byte offsets in the cross-reference table at the
//! bottom of the file, so a PDF is a document whose *contents encode their own
//! positions* — and a binary blob nobody in this repository can read would turn
//! any test that broke against it into an unfixable one. Every offset below is
//! computed from the bytes as they are written, in eleven lines, in the open.
//!
//! Shared between `crate::pdf`'s tests (which count the pages), `pages`' tests
//! (which draw them) and `screens::song_detail`'s (which show one), the way
//! `crate::picker::test_support` is shared, so that three test modules cannot
//! drift into three subtly different ideas of what a PDF is.

/// A valid PDF of `pages` pages, each US Letter and each with its **left half
/// filled black**.
///
/// The ink is the point. A page hayro failed to draw and a page hayro drew
/// correctly are both "a bitmap of the right size" from the outside, and
/// `docs/PDF.md` recorded exactly that failure in PDFium — `PdfBitmapFormat::
/// Gray` returning a correctly sized, entirely white page rather than an error.
/// A test that only checked dimensions would have passed against it. Half the
/// page in the default fill colour, which PDF says is black, is the cheapest
/// mark that cannot be confused with a blank.
///
/// `pages` of zero is a legal document with an empty page tree, which is the
/// case `page_count` deliberately reports as `None`.
pub fn inked_pdf(pages: usize) -> Vec<u8> {
    // One shared content stream for every page: `re` builds a rectangle over
    // the left half of a 612 x 792 box, `f` fills it.
    let content: &[u8] = b"0 0 306 792 re f\n";
    let content_object = 3;
    let first_page_object = 4;

    let kids: String = (0..pages)
        .map(|n| format!("{} 0 R ", first_page_object + n))
        .collect();

    let mut objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!("<< /Type /Pages /Kids [{kids}] /Count {pages} >>").into_bytes(),
        {
            let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
            stream.extend_from_slice(content);
            stream.extend_from_slice(b"endstream");
            stream
        },
    ];
    for _ in 0..pages {
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
                 /Resources << >> /Contents {content_object} 0 R >>"
            )
            .into_bytes(),
        );
    }

    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }

    // The table hayro seeks to, and the `startxref` that says where it is. Both
    // are why this is generated rather than typed: every number here is the
    // length of what came before it.
    let xref_at = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}
