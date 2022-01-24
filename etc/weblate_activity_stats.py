# <https://gitlab.com/Dimava/crosscode-translation-ru/-/blob/master/assets/editor/statPlot.js>

from weblate.trans.models import Project, Change
from weblate.lang.models import Language
import tqdm
import plotly.graph_objects
from plotly.offline import get_plotlyjs_version
from datetime import timedelta

interesting_actions = {
  # Change.ACTION_UPDATE,
  # Change.ACTION_COMPLETE,
  Change.ACTION_CHANGE,
  Change.ACTION_NEW,
  Change.ACTION_COMMENT,
  Change.ACTION_SUGGESTION,
  # Change.ACTION_AUTO,
  Change.ACTION_ACCEPT,
  Change.ACTION_REVERT,
  # Change.ACTION_UPLOAD,
  # Change.ACTION_NEW_SOURCE,
  # Change.ACTION_LOCK,
  # Change.ACTION_UNLOCK,
  # Change.ACTION_DUPLICATE_STRING,
  # Change.ACTION_COMMIT,
  # Change.ACTION_PUSH,
  # Change.ACTION_RESET,
  # Change.ACTION_MERGE,
  # Change.ACTION_REBASE,
  # Change.ACTION_FAILED_MERGE,
  # Change.ACTION_FAILED_REBASE,
  # Change.ACTION_FAILED_PUSH,
  # Change.ACTION_PARSE_ERROR,
  # Change.ACTION_REMOVE_TRANSLATION,
  Change.ACTION_SUGGESTION_DELETE,
  Change.ACTION_REPLACE,
  Change.ACTION_SUGGESTION_CLEANUP,
  Change.ACTION_SOURCE_CHANGE,
  Change.ACTION_NEW_UNIT,
  Change.ACTION_BULK_EDIT,
  # Change.ACTION_ACCESS_EDIT,
  # Change.ACTION_ADD_USER,
  # Change.ACTION_REMOVE_USER,
  Change.ACTION_APPROVE,
  Change.ACTION_MARKED_EDIT,
  # Change.ACTION_REMOVE_COMPONENT,
  # Change.ACTION_REMOVE_PROJECT,
  # Change.ACTION_DUPLICATE_LANGUAGE,
  # Change.ACTION_RENAME_PROJECT,
  # Change.ACTION_RENAME_COMPONENT,
  # Change.ACTION_MOVE_COMPONENT,
  # Change.ACTION_NEW_STRING,
  # Change.ACTION_NEW_CONTRIBUTOR,
  Change.ACTION_ANNOUNCEMENT,
  # Change.ACTION_ALERT,
  # Change.ACTION_ADDED_LANGUAGE,
  # Change.ACTION_REQUESTED_LANGUAGE,
  # Change.ACTION_CREATE_PROJECT,
  # Change.ACTION_CREATE_COMPONENT,
  # Change.ACTION_INVITE_USER,
  # Change.ACTION_HOOK,
  # Change.ACTION_REPLACE_UPLOAD,
  # Change.ACTION_LICENSE_CHANGE,
  # Change.ACTION_AGREEMENT_CHANGE,
  Change.ACTION_SCREENSHOT_ADDED,
  Change.ACTION_SCREENSHOT_UPLOADED,
}

queryset = (
  Change.objects
    .filter(project__slug="crosscode")
    .order_by("timestamp")
    .select_related("user", "language")
)

timestamps_by_user_by_language = {}
min_timestamp = None
max_timestamp = None
for change in tqdm.tqdm(queryset.iterator(), total=queryset.count()):
  if change.action not in interesting_actions:
    continue
  lang = change.language and change.language.code
  user = change.user and change.user.username
  time = change.timestamp
  timestamps_by_user_by_language.setdefault(lang, {}).setdefault(user, []).append(time)
  max_timestamp = max(max_timestamp, time) if max_timestamp else time
  min_timestamp = min(min_timestamp, time) if min_timestamp else time

figures = []
for lang, timestamps_by_user in timestamps_by_user_by_language.items():
  figure = plotly.graph_objects.Figure()
  lang_name = Language.objects.get(code=lang).get_name() if lang else "<none>"
  figure.update_layout(
    barmode="stack",
    hovermode="x",
    bargap=0,
    title=f"Language: {lang_name}",
    xaxis_range=[min_timestamp - timedelta(days=1.5), max_timestamp + timedelta(days=1)],
    showlegend=True,
  )
  for user, timestamps in timestamps_by_user.items():
    if not user:
      continue
    figure.add_histogram(
      name=user,
      x=timestamps,
      xbins_size=24 * 60 * 60 * 1000,
    )
  figures.append(figure)

html = "\n".join(figure.to_html(full_html=False, include_plotlyjs=False) for figure in figures)
html = f"""
<html>
<head>
<meta charset="utf-8">
</head>
<body>
<script src="https://cdn.plot.ly/plotly-{get_plotlyjs_version()}.min.js"></script>
{html}
</body>
</html>
"""
with open("/home/weblate/public/user_activity_plot.html", "w") as file:
  file.write(html)

# change_actions = {
#   v: k[7:] for k, v in vars(Change).items() if k.startswith("ACTION_") and type(v) == int
# }
#
# project = Project.objects.get(slug="crosscode")
# queryset = Change.objects.filter(project=project).order_by("timestamp")
# queryset = queryset.select_related(
#   "project",
#   "component",
#   "translation",
#   "translation__language",
#   "unit",
#   "user",
#   "author",
# )
#
# for change in queryset.iterator():
#
#   row = {
#     "id": change.id,
#     "timestamp": change.timestamp.replace(tzinfo=timezone.utc).timestamp(),
#     "action": change_actions[change.action],
#     "details": change.details,
#     "old": change.old,
#     "target": change.target,
#   }
#   if change.project:
#     row["project"] = change.project.slug
#   if change.component:
#     row["component"] = change.component.slug
#   if change.translation:
#     row["translation"] = change.translation.language.code
#   if change.unit:
#     row["unit"] = change.unit.id
#   if change.user:
#     row["user"] = change.user.username
#   if change.author:
#     row["author"] = change.author.username
#
#   try:
#     json.dump(row, sys.stdout, separators=(",", ":"))
#     sys.stdout.write("\n")
#   except BrokenPipeError:
#     break
