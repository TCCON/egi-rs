{GGGPATH}/bin/collate_tccon_results t
{GGGPATH}/bin/collate_tccon_results v
{GGGPATH}/bin/average_results {RUNLOG}.tsw
{GGGPATH}/bin/average_results {RUNLOG}.vsw
{GGGPATH}/bin/apply_tccon_airmass_correction {GGGPATH}/tccon/corrections_airmass_postavg.em27.dat {RUNLOG}.vav
{GGGPATH}/bin/apply_tccon_insitu_correction {GGGPATH}/tccon/corrections_insitu_postavg.em27.dat {RUNLOG}.vav.ada
{GGGPATH}/bin/error_scale_factor {RUNLOG}.vav.ada.aia
{GGGPATH}/bin/extract_pth {RUNLOG}.grl y
{GGGPATH}/bin/write_official_output_file {RUNLOG}.vav.ada.aia
{GGGPATH}/bin/apply_manual_flags {RUNLOG}.vav.ada.aia.oof
{GGGPATH}/bin/write_netcdf {RUNLOG}.tav
{GGGPATH}/bin/add_nc_flags json -i --nc-file {SITE_ID}*.private.nc {GGGPATH}/tccon/{SITE_ID}_extra_filters.json
{GGGPATH}/bin/write_aux {RUNLOG}.mav
