/* ЛБ1, задача 3: формула и проверка условия.
 * Перевод градусов в радианы, котангенс через тангенс. */
#include <stdio.h>
#include <math.h>

#define PI 3.14159265358979323846

int main(void) {
    double deg = 0.0;
    double rad = 0.0;
    double tg = 0.0;

    printf("угол в градусах: ");
    scanf("%lf", &deg);
    rad = deg * PI / 180.0;
    tg = tan(rad);
    printf("радианы = %.4lf\n", rad);
    if (tg == 0.0) {
        printf("котангенс не определён\n");
    } else {
        printf("котангенс = %.4lf\n", 1.0 / tg);
    }
    if (deg >= 0.0 && deg <= 90.0) {
        printf("угол в первой четверти\n");
    } else {
        printf("угол вне первой четверти\n");
    }
    return 0;
}
