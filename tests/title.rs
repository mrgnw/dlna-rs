use dlna_rs::library::clean_title;

#[test]
fn movie_keeps_title_and_year_drops_release_tags() {
	assert_eq!(
		clean_title("Anaconda.2025.1080p.MA.WEB-DL.DDP5.1.Atmos.H.264-BYNDR.mkv"),
		"Anaconda (2025)"
	);
	assert_eq!(
		clean_title("The.Matrix.1999.1080p.BluRay.x264-GRP.mkv"),
		"The Matrix (1999)"
	);
}

#[test]
fn movie_with_no_release_tags_still_formats_year() {
	assert_eq!(
		clean_title("Before.Sunset.2004.mp4"),
		"Before Sunset (2004)"
	);
}

#[test]
fn parenthesised_year_is_preserved_without_doubling() {
	assert_eq!(
		clean_title("Broken.Flowers (2005).mp4"),
		"Broken Flowers (2005)"
	);
}

#[test]
fn tv_episode_keeps_season_episode_marker() {
	assert_eq!(
		clean_title("Show.Name.S01E02.1080p.WEB.mkv"),
		"Show Name S01E02"
	);
}

#[test]
fn clean_name_is_left_alone() {
	assert_eq!(clean_title("Inception.mkv"), "Inception");
	assert_eq!(clean_title("My Home Video.mp4"), "My Home Video");
}

#[test]
fn numeric_title_is_not_mistaken_for_year() {
	// Title "2012", real year 2009 — the year is the token before the first release tag.
	assert_eq!(
		clean_title("2012.2009.1080p.BluRay.x264.mkv"),
		"2012 (2009)"
	);
	// "Apollo 13" — 13 is part of the title, 1995 is the year.
	assert_eq!(
		clean_title("Apollo.13.1995.1080p.BluRay.mkv"),
		"Apollo 13 (1995)"
	);
}

#[test]
fn falls_back_to_filename_stem_when_cleaning_empties_it() {
	assert_eq!(clean_title("___.mkv"), "___");
}
